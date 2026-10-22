//! Proc macros behind the `svelters` facade (E0 of `docs/notes/egui-rust-exports.md`). The generated
//! code only names `::svelters::__private::*` (the runtime in `svelters_host::exports`).

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Ts};
use quote::{format_ident, quote, quote_spanned};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, FnArg, ImplItem, ItemFn, ItemImpl, ItemStruct, Lit, Meta, Pat, ReturnType, Token, Type};

// ---- naming ----

fn camel(snake: &str) -> String {
    let s = snake.trim_start_matches("r#");
    let mut out = String::new();
    let mut up = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            up = i > 0;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn kebab(pascal: &str) -> String {
    let mut out = String::new();
    for (i, c) in pascal.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else if c == '_' {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    out
}

fn docs(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for a in attrs {
        if a.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &a.meta {
                if let Expr::Lit(l) = &nv.value {
                    if let Lit::Str(s) = &l.lit {
                        lines.push(s.value().trim().to_string());
                    }
                }
            }
        }
    }
    lines.join("\n").trim().to_string()
}

// ---- #[js(...)] options ----

#[derive(Default)]
struct Js {
    constructor: bool,
    getter: bool,
    setter: bool,
    is_static: bool,
    skip: bool,
    bigint: bool,
    rename: Option<String>,
    default: Option<Option<Expr>>,
    code: Option<String>,
    // E1 offload
    worker: bool,
    blocking: bool,
    local: bool,
    tokio: bool,
    supersede: bool,
    max_in_flight: Option<u32>,
    // E3 streams
    buffer: Option<usize>,
    // RA2 data enums
    tag: Option<String>,
}

fn js_opts(attrs: &[Attribute]) -> syn::Result<Js> {
    let mut js = Js::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("js")) {
        a.parse_nested_meta(|m| {
            let id = m.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            match id.as_str() {
                "constructor" => js.constructor = true,
                "getter" => js.getter = true,
                "setter" => js.setter = true,
                "static" => js.is_static = true,
                "skip" => js.skip = true,
                "bigint" => js.bigint = true,
                "rename" => js.rename = Some(m.value()?.parse::<syn::LitStr>()?.value()),
                "code" => js.code = Some(m.value()?.parse::<syn::LitStr>()?.value()),
                "default" => {
                    js.default = Some(if m.input.peek(Token![=]) { Some(m.value()?.parse::<Expr>()?) } else { None });
                }
                "worker" => js.worker = true,
                "blocking" => js.blocking = true,
                "local" => js.local = true,
                "tokio" => js.tokio = true,
                "supersede" => js.supersede = true,
                "max_in_flight" => {
                    let n = m.value()?.parse::<syn::LitInt>()?.base10_parse::<u32>()?;
                    if n == 0 {
                        return Err(m.error("max_in_flight must be at least 1"));
                    }
                    js.max_in_flight = Some(n);
                }
                "buffer" => {
                    let n = m.value()?.parse::<syn::LitInt>()?.base10_parse::<usize>()?;
                    if n == 0 {
                        return Err(m.error("a stream's buffer must hold at least 1 item"));
                    }
                    js.buffer = Some(n);
                }
                "tag" => {
                    let t = m.value()?.parse::<syn::LitStr>()?.value();
                    if t.is_empty() {
                        return Err(m.error("the tag key cannot be empty"));
                    }
                    js.tag = Some(t);
                }
                _ => return Err(m.error(format!("unknown #[js] option `{id}`"))),
            }
            Ok(())
        })?;
    }
    Ok(js)
}

fn strip_js(attrs: &mut Vec<Attribute>) {
    attrs.retain(|a| !a.path().is_ident("js"));
}

fn last_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// The first type argument of the last path segment (`T` of `Sink<T>`).
fn generic_arg(ty: &Type) -> Option<Type> {
    let Type::Path(p) = ty else { return None };
    let syn::PathArguments::AngleBracketed(a) = &p.path.segments.last()?.arguments else { return None };
    a.args.iter().find_map(|x| match x {
        syn::GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    })
}

fn pascal(snake: &str) -> String {
    let c = camel(snake);
    let mut it = c.chars();
    match it.next() {
        Some(f) => f.to_uppercase().chain(it).collect(),
        None => c,
    }
}

// ---- derives ----

/// `#[derive(svelters::Record)]` on a struct with named fields: a plain JS object (copied).
/// Fields are camelCase (`#[js(rename)]`); `Option<T>` and `#[js(default [= expr])]` fields are optional.
#[proc_macro_derive(Record, attributes(js))]
pub fn derive_record(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    record(input).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn record(input: DeriveInput) -> syn::Result<Ts> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(input.generics.span(), "exported records cannot be generic"));
    }
    let Data::Struct(ds) = &input.data else {
        return Err(syn::Error::new(input.span(), "#[derive(svelters::Record)] needs a struct with named fields (a data enum is `#[derive(svelters::Enum)]` with `#[js(tag = \"kind\")]`)"));
    };
    let Fields::Named(fields) = &ds.fields else {
        return Err(syn::Error::new(ds.fields.span(), "#[derive(svelters::Record)] needs named fields"));
    };
    let p = quote!(::svelters::__private);
    let jsname = js_opts(&input.attrs)?.rename.unwrap_or_else(|| name.to_string());
    let tdocs = docs(&input.attrs);
    let mut names = Vec::new();
    let mut descs = Vec::new();
    let mut froms = Vec::new();
    let mut intos = Vec::new();
    for f in &fields.named {
        let o = js_opts(&f.attrs)?;
        let id = f.ident.as_ref().expect("named");
        let ty = &f.ty;
        let fname = o.rename.clone().unwrap_or_else(|| camel(&id.to_string()));
        let fdocs = docs(&f.attrs);
        let optional = o.default.is_some() || last_ident(ty).as_deref() == Some("Option");
        let mut d = quote!(<#ty as #p::JsType>::describe(b));
        if o.bigint {
            d = quote!(#p::__private::bigint(#d));
        }
        descs.push(quote!(#p::FieldDef { name: #fname, ty: #d, optional: #optional, docs: #fdocs }));
        names.push(quote!((#fname, #optional)));
        let at = quote!(.map_err(|e| #p::__private::at(e, #fname))?);
        froms.push(match &o.default {
            Some(Some(e)) => quote!(#id: #p::__private::or_default::<#ty>(__f.next().unwrap_or(#p::Value::Null), cx, || #e)#at),
            Some(None) => quote!(#id: #p::__private::or_default::<#ty>(__f.next().unwrap_or(#p::Value::Null), cx, ::core::default::Default::default)#at),
            None => quote!(#id: <#ty as #p::FromJs>::from_js(__f.next().unwrap_or(#p::Value::Null), cx)#at),
        });
        intos.push(quote!(#p::IntoJs::into_js(self.#id, cx)?));
    }
    Ok(quote! {
        impl #p::JsType for #name {
            fn describe(b: &mut #p::TypeBuilder) -> #p::TypeRef {
                b.record::<Self>(#jsname, #tdocs, |b| ::std::vec![#(#descs),*])
            }
        }
        impl #p::FromJs for #name {
            fn from_js(v: #p::Value, cx: &#p::CallCx<'_>) -> ::core::result::Result<Self, #p::NativeError> {
                let mut __f = #p::__private::record_fields(v, &[#(#names),*])?;
                ::core::result::Result::Ok(#name { #(#froms),* })
            }
        }
        impl #p::IntoJs for #name {
            fn into_js(self, cx: &#p::CallCx<'_>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                ::core::result::Result::Ok(#p::Value::Record { ty: #p::__private::record_type::<Self>(cx)?, fields: ::std::vec![#(#intos),*] })
            }
        }
    })
}

/// `#[derive(svelters::Enum)]` on a unit-only enum: a string-literal union of the kebab-case variant
/// names (`#[js(rename)]` per variant). With `#[js(tag = "kind")]` (RA2) the variants may carry data:
/// a discriminated union of plain objects, serde's internally tagged form (`{ kind: 'circle', r: 1 }`).
#[proc_macro_derive(Enum, attributes(js))]
pub fn derive_enum(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    enumeration(input).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn enumeration(input: DeriveInput) -> syn::Result<Ts> {
    let name = &input.ident;
    let Data::Enum(de) = &input.data else {
        return Err(syn::Error::new(input.span(), "#[derive(svelters::Enum)] needs an enum"));
    };
    let p = quote!(::svelters::__private);
    let topts = js_opts(&input.attrs)?;
    if let Some(tag) = topts.tag {
        return tagged_enum(&input, de, tag, topts.rename);
    }
    let jsname = topts.rename.unwrap_or_else(|| name.to_string());
    let tdocs = docs(&input.attrs);
    let mut values = Vec::new();
    let mut from = Vec::new();
    let mut into = Vec::new();
    for (i, v) in de.variants.iter().enumerate() {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(v.span(), "#[derive(svelters::Enum)] variants must be unit variants; a data enum needs `#[js(tag = \"kind\")]` on the enum"));
        }
        let o = js_opts(&v.attrs)?;
        values.push(o.rename.unwrap_or_else(|| kebab(&v.ident.to_string())));
        let id = &v.ident;
        let i = i as i64;
        from.push(quote!(#i => #name::#id));
        into.push(quote!(#name::#id => #i));
    }
    Ok(quote! {
        impl #p::JsType for #name {
            fn describe(b: &mut #p::TypeBuilder) -> #p::TypeRef {
                b.enumeration::<Self>(#jsname, #tdocs, &[#(#values),*])
            }
        }
        impl #p::FromJs for #name {
            fn from_js(v: #p::Value, _cx: &#p::CallCx<'_>) -> ::core::result::Result<Self, #p::NativeError> {
                ::core::result::Result::Ok(match #p::__private::enum_value(v, &[#(#values),*])? {
                    #(#from,)*
                    other => return ::core::result::Result::Err(#p::__private::bad_enum(#jsname, other)),
                })
            }
        }
        impl #p::IntoJs for #name {
            fn into_js(self, cx: &#p::CallCx<'_>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                ::core::result::Result::Ok(#p::Value::Enum { ty: #p::__private::record_type::<Self>(cx)?, value: match self { #(#into),* } })
            }
        }
    })
}

/// RA2: `#[derive(svelters::Enum)] #[js(tag = "kind")]`: unit, struct and newtype variants (a newtype's
/// inner type is a record or another object: its fields sit next to the tag).
fn tagged_enum(input: &DeriveInput, de: &syn::DataEnum, tag: String, rename: Option<String>) -> syn::Result<Ts> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(input.generics.span(), "exported enums cannot be generic"));
    }
    let p = quote!(::svelters::__private);
    let jsname = rename.unwrap_or_else(|| name.to_string());
    let tdocs = docs(&input.attrs);
    let mut variants = Vec::new();
    let mut tags = Vec::new();
    let mut from = Vec::new();
    let mut into = Vec::new();
    for v in &de.variants {
        let o = js_opts(&v.attrs)?;
        let vtag = o.rename.unwrap_or_else(|| kebab(&v.ident.to_string()));
        if tags.contains(&vtag) {
            return Err(syn::Error::new(v.span(), format!("two variants have the tag `{vtag}`")));
        }
        tags.push(vtag.clone());
        let id = &v.ident;
        let vdocs = docs(&v.attrs);
        match &v.fields {
            Fields::Unit => {
                variants.push(quote!(#p::VariantDef { name: #vtag, docs: #vdocs, fields: ::std::vec::Vec::new(), inner: ::core::option::Option::None }));
                from.push(quote!(#vtag => #name::#id));
                into.push(quote!(#name::#id => #p::__private::tagged_value(#tag, #vtag, ::std::vec::Vec::new())));
            }
            Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                let ty = &u.unnamed[0].ty;
                variants.push(quote!(#p::VariantDef { name: #vtag, docs: #vdocs, fields: ::std::vec::Vec::new(), inner: ::core::option::Option::Some(<#ty as #p::JsType>::describe(b)) }));
                from.push(quote!(#vtag => #name::#id(__m.rest::<#ty>(cx)?)));
                into.push(quote!(#name::#id(__x) => #p::__private::tagged_inner(#tag, #vtag, #p::IntoJs::into_js(__x, cx)?, cx)?));
            }
            Fields::Unnamed(u) => {
                return Err(syn::Error::new(u.span(), "a tagged variant is a unit, a struct variant or a newtype around a record (serde's internally tagged form has no tuples)"));
            }
            Fields::Named(fields) => {
                let mut fdefs = Vec::new();
                let mut reads = Vec::new();
                let mut binds = Vec::new();
                let mut writes = Vec::new();
                for f in &fields.named {
                    let fo = js_opts(&f.attrs)?;
                    if fo.bigint {
                        return Err(syn::Error::new(f.span(), "#[js(bigint)] is not supported inside a tagged variant (it crosses as a plain object): use a record field"));
                    }
                    let fid = f.ident.as_ref().expect("named");
                    let ty = &f.ty;
                    let fname = fo.rename.clone().unwrap_or_else(|| camel(&fid.to_string()));
                    if fname == tag {
                        return Err(syn::Error::new(f.span(), format!("a field of a tagged variant cannot be named like the tag `{tag}`")));
                    }
                    let fdocs = docs(&f.attrs);
                    let optional = fo.default.is_some() || last_ident(ty).as_deref() == Some("Option");
                    fdefs.push(quote!(#p::FieldDef { name: #fname, ty: <#ty as #p::JsType>::describe(b), optional: #optional, docs: #fdocs }));
                    reads.push(match &fo.default {
                        Some(Some(e)) => quote!(#fid: __m.field_or::<#ty>(#fname, cx, || #e)?),
                        Some(None) => quote!(#fid: __m.field_or::<#ty>(#fname, cx, ::core::default::Default::default)?),
                        None => quote!(#fid: __m.field::<#ty>(#fname, #optional, cx)?),
                    });
                    binds.push(quote!(#fid));
                    writes.push(quote!((#fname, #p::IntoJs::into_js(#fid, cx)?)));
                }
                variants.push(quote!(#p::VariantDef { name: #vtag, docs: #vdocs, fields: ::std::vec![#(#fdefs),*], inner: ::core::option::Option::None }));
                from.push(quote!(#vtag => #name::#id { #(#reads),* }));
                into.push(quote!(#name::#id { #(#binds),* } => #p::__private::tagged_value(#tag, #vtag, ::std::vec![#(#writes),*])));
            }
        }
    }
    Ok(quote! {
        impl #p::JsType for #name {
            fn describe(b: &mut #p::TypeBuilder) -> #p::TypeRef {
                b.union::<Self>(#jsname, #tdocs, #tag, |b| ::std::vec![#(#variants),*])
            }
        }
        impl #p::FromJs for #name {
            fn from_js(v: #p::Value, cx: &#p::CallCx<'_>) -> ::core::result::Result<Self, #p::NativeError> {
                #[allow(unused_mut)]
                let mut __m = #p::__private::tagged(v, #tag, #jsname)?;
                let __t = ::core::mem::take(&mut __m.tag);
                let __r = match __t.as_str() {
                    #(#from,)*
                    _ => return ::core::result::Result::Err(__m.unknown(&__t, &[#(#tags),*])),
                };
                ::core::result::Result::Ok(__r)
            }
        }
        impl #p::IntoJs for #name {
            fn into_js(self, cx: &#p::CallCx<'_>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                let _ = cx;
                ::core::result::Result::Ok(match self { #(#into),* })
            }
        }
    })
}

/// `#[derive(svelters::Error)]`: the type rejects/throws as `NativeError { domain: 'app:<module>',
/// code: '<kebab variant>' (or `#[js(code = "...")]`), message: Display }`. Needs `Display`.
#[proc_macro_derive(Error, attributes(js))]
pub fn derive_error(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    error(input).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn error(input: DeriveInput) -> syn::Result<Ts> {
    let name = &input.ident;
    let p = quote!(::svelters::__private);
    let (gi, gt, gw) = input.generics.split_for_impl();
    let (code, codes) = match &input.data {
        Data::Enum(de) => {
            let mut arms = Vec::new();
            let mut codes = Vec::new();
            for v in &de.variants {
                let c = js_opts(&v.attrs)?.code.unwrap_or_else(|| kebab(&v.ident.to_string()));
                let id = &v.ident;
                let pat = match &v.fields {
                    Fields::Unit => quote!(#name::#id),
                    Fields::Unnamed(_) => quote!(#name::#id(..)),
                    Fields::Named(_) => quote!(#name::#id { .. }),
                };
                arms.push(quote!(#pat => #c));
                codes.push(c);
            }
            (quote!(match self { #(#arms),* }.to_string()), codes)
        }
        _ => {
            let c = js_opts(&input.attrs)?.code.unwrap_or_else(|| kebab(&name.to_string()));
            (quote!(#c.to_string()), vec![c])
        }
    };
    Ok(quote! {
        impl #gi #p::ExportError for #name #gt #gw {
            fn code(&self) -> ::std::string::String { #code }
            fn message(&self) -> ::std::string::String { ::std::string::ToString::to_string(self) }
            fn codes() -> ::std::vec::Vec<&'static str> { ::std::vec![#(#codes),*] }
        }
    })
}

// ---- #[svelters::object] ----

struct ObjectArgs {
    shared: bool,
    module: Option<String>,
    name: Option<String>,
}

fn object_args(attr: Ts) -> syn::Result<ObjectArgs> {
    let mut a = ObjectArgs { shared: false, module: None, name: None };
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr)?;
    for m in metas {
        match &m {
            Meta::Path(p) if p.is_ident("shared") => a.shared = true,
            Meta::Path(p) if p.is_ident("ui") => a.shared = false,
            Meta::NameValue(nv) if nv.path.is_ident("module") || nv.path.is_ident("name") => {
                let Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value else {
                    return Err(syn::Error::new(nv.value.span(), "expected a string"));
                };
                if nv.path.is_ident("module") {
                    a.module = Some(s.value());
                } else {
                    a.name = Some(s.value());
                }
            }
            _ => return Err(syn::Error::new(m.span(), "expected `shared`, `module = \"...\"` or `name = \"...\"`")),
        }
    }
    Ok(a)
}

fn check_module(m: &str, span: Span) -> syn::Result<()> {
    if m.is_empty() || !m.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err(syn::Error::new(span, "module names are lower-case identifiers: `svelters:app/<module>`"));
    }
    Ok(())
}

/// `#[svelters::object(module = "m")]` (UI thread, `Rc<RefCell<Self>>`, `&mut self` methods) or
/// `#[svelters::object(shared, module = "m")]` (`Arc<Self>`, `Send + Sync` checked). The JS class is
/// `svelters:app/m`'s `<Name>` (`name = "..."` renames it).
#[proc_macro_attribute]
pub fn object(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemStruct);
    object_impl(attr.into(), item).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn object_impl(attr: Ts, item: ItemStruct) -> syn::Result<Ts> {
    let a = object_args(attr)?;
    let module = a.module.ok_or_else(|| syn::Error::new(Span::call_site(), "#[svelters::object] needs `module = \"...\"` (the JS module is `svelters:app/<module>`)"))?;
    check_module(&module, item.span())?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(item.generics.span(), "exported objects cannot be generic"));
    }
    let ty = &item.ident;
    let rust = ty.to_string();
    let jsname = a.name.unwrap_or_else(|| rust.clone());
    let odocs = docs(&item.attrs);
    let shared = a.shared;
    let p = quote!(::svelters::__private);
    let flavor = if shared {
        let assert = quote_spanned!(ty.span()=> const _: () = #p::__private::assert_shared::<#ty>(););
        quote! {
            #assert
            impl #p::ExportObject for #ty {
                const MODULE: &'static str = #module;
                const NAME: &'static str = #jsname;
                const SHARED: bool = true;
                const DOCS: &'static str = #odocs;
                type Holder = ::std::sync::Arc<#ty>;
                type Ref<'a> = &'a #ty;
                fn __holder(o: &#p::AnyObj) -> ::core::option::Option<Self::Holder> { #p::__private::shared_holder::<#ty>(o) }
                fn __borrow(h: &Self::Holder) -> ::core::result::Result<Self::Ref<'_>, #p::NativeError> { ::core::result::Result::Ok(&**h) }
                fn __erase(h: Self::Holder) -> #p::AnyObj { #p::AnyObj::Shared(h) }
                fn __new(v: Self) -> Self::Holder { ::std::sync::Arc::new(v) }
            }
            impl #p::SharedObject for #ty {}
        }
    } else {
        quote! {
            impl #p::ExportObject for #ty {
                const MODULE: &'static str = #module;
                const NAME: &'static str = #jsname;
                const SHARED: bool = false;
                const DOCS: &'static str = #odocs;
                type Holder = ::std::rc::Rc<::std::cell::RefCell<#ty>>;
                type Ref<'a> = ::std::cell::Ref<'a, #ty>;
                fn __holder(o: &#p::AnyObj) -> ::core::option::Option<Self::Holder> { #p::__private::ui_holder::<#ty>(o) }
                fn __borrow(h: &Self::Holder) -> ::core::result::Result<Self::Ref<'_>, #p::NativeError> { #p::__private::ui_borrow::<#ty>(h) }
                fn __erase(h: Self::Holder) -> #p::AnyObj { #p::AnyObj::Ui(h) }
                fn __new(v: Self) -> Self::Holder { ::std::rc::Rc::new(::std::cell::RefCell::new(v)) }
            }
            impl #p::UiObject for #ty {}
        }
    };
    // E3: an `Emitter<E>` field gives the class `E`'s events (`obj.on(name, fn)`).
    let mut emitters = Vec::new();
    for f in item.fields.iter() {
        if last_ident(&f.ty).as_deref() == Some("Emitter") {
            let ev = generic_arg(&f.ty).ok_or_else(|| syn::Error::new(f.ty.span(), "expected `Emitter<E>`"))?;
            let fname = match &f.ident {
                Some(i) => quote!(#i),
                None => return Err(syn::Error::new(f.span(), "an `Emitter<E>` field needs a name")),
            };
            emitters.push((ev, fname));
        }
    }
    if emitters.len() > 1 {
        return Err(syn::Error::new(item.span(), "an exported object has at most one `Emitter<E>` field (put every event in one enum)"));
    }
    let emitter = emitters.first().map(|(ev, fname)| {
        quote! {
            #p::inventory::submit! {
                #p::__private::EmitterDef {
                    type_id: || ::std::any::TypeId::of::<#ty>(),
                    events: <#ev as #p::EventSet>::EVENTS,
                    describe: <#ev as #p::EventSet>::__describe,
                    emitter: |o| #p::__private::emitter_of::<#ty>(o, |s: &#ty| s.#fname.__core()),
                }
            }
        }
    });
    Ok(quote! {
        #item
        #flavor
        #emitter
        impl #p::JsType for #ty {
            fn describe(b: &mut #p::TypeBuilder) -> #p::TypeRef { b.class::<#ty>() }
        }
        impl #p::IntoJs for #ty {
            fn into_js(self, cx: &#p::CallCx<'_>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                cx.adopt::<#ty>(<#ty as #p::ExportObject>::__new(self))
            }
        }
        #p::inventory::submit! {
            #p::ObjectDef { module: #module, name: #jsname, rust: #rust, docs: #odocs, shared: #shared, type_id: || ::std::any::TypeId::of::<#ty>() }
        }
    })
}

// ---- #[svelters::export] ----

struct ReplaceSelf<'a>(&'a Type);
impl VisitMut for ReplaceSelf<'_> {
    fn visit_type_mut(&mut self, t: &mut Type) {
        if let Type::Path(tp) = t {
            if tp.qself.is_none() && tp.path.is_ident("Self") {
                *t = self.0.clone();
                return;
            }
        }
        syn::visit_mut::visit_type_mut(self, t);
    }
}

enum ArgKind {
    /// FromJs by value
    Value(Type),
    /// `&str` -> String, `&[T]` -> Vec<T>: converted, passed by reference.
    ByRef(Type),
    /// `&T` of an exported object
    Obj(Type),
    /// `&mut T` of an exported UI object
    ObjMut(Type),
    /// `WorkCx<P>`: injected by the host (offloaded members), not a JS argument.
    Injected(Type),
    /// E3: `Sink<T>` (the item type): a stream bound to the call, injected by the host.
    Sink(Type),
}

struct Arg {
    name: String,
    kind: ArgKind,
    bigint: bool,
}

/// Generates the thunk, the signature function and the `MemberDef` of one exported function.
struct Gen {
    defs: Vec<Ts>,
    items: Vec<Ts>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recv {
    None,
    Ref,
    Mut,
    Arc,
}

/// Where an offloaded member runs (E1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Exec {
    Worker,
    Blocking,
    Local,
    Tokio,
}

fn exec_of(sig: &syn::Signature, js: &Js) -> syn::Result<Option<Exec>> {
    let span = sig.ident.span();
    let picked = [js.worker, js.blocking, js.local, js.tokio].iter().filter(|x| **x).count();
    if picked > 1 {
        return Err(syn::Error::new(span, "pick one of #[js(worker)], #[js(blocking)], #[js(local)], #[js(tokio)]"));
    }
    let exec = if sig.asyncness.is_some() {
        if js.worker || js.blocking {
            return Err(syn::Error::new(
                span,
                "#[js(worker)]/#[js(blocking)] run a synchronous fn on a pool; an `async fn` runs on the JS thread (default, or #[js(local)]) or on tokio (#[js(tokio)])",
            ));
        }
        Some(if js.tokio { Exec::Tokio } else { Exec::Local })
    } else if js.tokio {
        return Err(syn::Error::new(span, "#[js(tokio)] needs an `async fn`"));
    } else if js.local {
        return Err(syn::Error::new(span, "#[js(local)] is for an `async fn`; a plain fn already runs on the JS thread, synchronously"));
    } else if js.worker {
        Some(Exec::Worker)
    } else if js.blocking {
        Some(Exec::Blocking)
    } else {
        None
    };
    if exec.is_none() && (js.supersede || js.max_in_flight.is_some()) {
        return Err(syn::Error::new(span, "#[js(supersede)] and #[js(max_in_flight)] apply to offloaded calls (#[js(worker)], #[js(blocking)], `async fn`)"));
    }
    if exec.is_some() && (js.constructor || js.getter || js.setter) {
        return Err(syn::Error::new(span, "constructors, getters and setters run synchronously: they cannot be offloaded or async"));
    }
    Ok(exec)
}

#[allow(clippy::too_many_arguments)]
fn member(
    g: &mut Gen,
    idx: usize,
    self_ty: Option<&Type>,
    sig: &mut syn::Signature,
    js: &Js,
    fdocs: &str,
    call_path: Ts,
) -> syn::Result<()> {
    let p = quote!(::svelters::__private);
    let exec = exec_of(sig, js)?;
    if !sig.generics.params.is_empty() {
        return Err(syn::Error::new(sig.generics.span(), "exported functions cannot be generic"));
    }
    let rust = sig.ident.to_string();
    let mut recv = Recv::None;
    let mut args = Vec::new();
    for (i, input) in sig.inputs.iter_mut().enumerate() {
        match input {
            FnArg::Receiver(r) => {
                if r.colon_token.is_some() {
                    // `self: Arc<Self>`
                    if last_ident(&r.ty).as_deref() != Some("Arc") {
                        return Err(syn::Error::new(r.ty.span(), "the only typed receiver an export takes is `self: Arc<Self>` (shared objects)"));
                    }
                    recv = Recv::Arc;
                } else if r.reference.is_none() {
                    return Err(syn::Error::new(r.span(), "exported methods cannot take `self` by value (the object stays owned by JS)"));
                } else if r.mutability.is_some() {
                    if matches!(exec, Some(Exec::Worker | Exec::Blocking | Exec::Tokio)) {
                        return Err(syn::Error::new(
                            r.span(),
                            "worker methods need #[svelters::object(shared)] and `self: Arc<Self>` or `&self` (Self: Send + Sync); `&mut self` stays on the JS thread (a plain `async fn`)",
                        ));
                    }
                    recv = Recv::Mut;
                } else {
                    recv = Recv::Ref;
                }
            }
            FnArg::Typed(pt) => {
                let o = js_opts(&pt.attrs)?;
                strip_js(&mut pt.attrs);
                let name = match &*pt.pat {
                    Pat::Ident(pi) => camel(&pi.ident.to_string()),
                    _ => format!("arg{i}"),
                };
                let mut ty = (*pt.ty).clone();
                if let Some(st) = self_ty {
                    ReplaceSelf(st).visit_type_mut(&mut ty);
                }
                let kind = match &ty {
                    Type::Path(_) if last_ident(&ty).as_deref() == Some("Sink") => {
                        if exec.is_none() {
                            return Err(syn::Error::new(
                                ty.span(),
                                "a `Sink<T>` parameter streams from offloaded work: add #[js(worker)] or #[js(blocking)], or make the fn `async` (use `sink.feed(x).await` there); a plain fn returns a `Stream<T>` instead",
                            ));
                        }
                        let item = generic_arg(&ty).ok_or_else(|| syn::Error::new(ty.span(), "expected `Sink<T>`"))?;
                        ArgKind::Sink(item)
                    }
                    Type::Path(_) if last_ident(&ty).as_deref() == Some("WorkCx") => {
                        if exec.is_none() {
                            return Err(syn::Error::new(ty.span(), "`WorkCx` is for offloaded calls: add #[js(worker)], #[js(blocking)], or make the fn `async`"));
                        }
                        ArgKind::Injected(ty)
                    }
                    Type::Reference(r) => {
                        if exec.is_some() {
                            return Err(syn::Error::new(
                                r.span(),
                                "an offloaded call outlives the JS call that started it, so it cannot borrow its arguments: take `String`, `Vec<T>`, `Arc<T>` or `Obj<T>`",
                            ));
                        }
                        let inner = (*r.elem).clone();
                        match &inner {
                            Type::Path(tp) if tp.path.is_ident("str") => {
                                if r.mutability.is_some() {
                                    return Err(syn::Error::new(r.span(), "`&mut str` cannot cross from JS"));
                                }
                                ArgKind::ByRef(syn::parse_quote!(::std::string::String))
                            }
                            Type::Slice(s) => {
                                if r.mutability.is_some() {
                                    return Err(syn::Error::new(r.span(), "`&mut [T]` cannot borrow a JS array: take `svelters::Buffer<T>` (the typed array is moved, zero-copy) and return it"));
                                }
                                let e = &s.elem;
                                ArgKind::ByRef(syn::parse_quote!(::std::vec::Vec<#e>))
                            }
                            _ if r.mutability.is_some() => ArgKind::ObjMut(inner),
                            _ => ArgKind::Obj(inner),
                        }
                    }
                    _ => ArgKind::Value(ty),
                };
                args.push(Arg { name, kind, bigint: o.bigint });
            }
        }
    }
    let kind = if js.constructor {
        quote!(Constructor)
    } else if js.getter {
        quote!(Getter)
    } else if js.setter {
        quote!(Setter)
    } else if self_ty.is_none() || recv == Recv::None {
        quote!(Static)
    } else {
        quote!(Method)
    };
    if (js.getter || js.setter) && recv == Recv::None {
        return Err(syn::Error::new(sig.span(), "getters and setters need a receiver (`&self`, or `&mut self` on UI objects)"));
    }
    if js.constructor && recv != Recv::None {
        return Err(syn::Error::new(sig.span(), "a #[js(constructor)] takes no receiver"));
    }
    let receiver = match recv {
        Recv::None => quote!(None),
        Recv::Ref => quote!(Ref),
        Recv::Mut => quote!(Mut),
        Recv::Arc => quote!(Arc),
    };
    let jsname = js.rename.clone().unwrap_or_else(|| {
        if js.setter {
            camel(rust.strip_prefix("set_").unwrap_or(&rust))
        } else {
            camel(&rust)
        }
    });

    let pooled = matches!(exec, Some(Exec::Worker | Exec::Blocking | Exec::Tokio));
    let mut conv = Vec::new();
    let mut borrows = Vec::new();
    let mut pass = Vec::new();
    let mut params = Vec::new();
    let mut checks = Vec::new();
    let mut progress = quote!(::core::option::Option::None);
    let mut items = quote!(::core::option::Option::None);
    let sinks = args.iter().filter(|a| matches!(a.kind, ArgKind::Sink(_))).count();
    if sinks > 1 {
        return Err(syn::Error::new(sig.span(), "an export streams through at most one `Sink<T>`"));
    }
    for (i, a) in args.iter().enumerate() {
        let v = format_ident!("__a{i}");
        let b = format_ident!("__b{i}");
        let next = quote!(__it.next().unwrap_or(#p::Value::Null));
        let name = &a.name;
        let dty = match &a.kind {
            ArgKind::Injected(t) => {
                conv.push(quote!(let #v = <#t as #p::__private::Injected>::inject(__op);));
                pass.push(quote!(#v));
                progress = quote!(<#t as #p::__private::Injected>::progress(b));
                continue;
            }
            ArgKind::Sink(t) => {
                let buf = match js.buffer {
                    Some(n) => quote!(#n),
                    None => quote!(#p::__private::DEFAULT_BUFFER),
                };
                conv.push(quote!(let #v = #p::__private::sink_for::<#t>(__op, #buf);));
                pass.push(quote!(#v));
                items = quote!(#p::__private::item_type::<#t>(b));
                if pooled {
                    checks.push(quote_spanned!(t.span()=> #p::__private::assert_worker_send::<#t>();));
                }
                continue;
            }
            ArgKind::Value(t) => {
                conv.push(quote!(let #v = <#t as #p::FromJs>::from_js(#next, cx).map_err(|e| #p::__private::at(e, #name))?;));
                pass.push(quote!(#v));
                if pooled {
                    checks.push(quote_spanned!(t.span()=> #p::__private::assert_worker_send::<#t>();));
                }
                t.clone()
            }
            ArgKind::ByRef(t) => {
                conv.push(quote!(let #v = <#t as #p::FromJs>::from_js(#next, cx).map_err(|e| #p::__private::at(e, #name))?;));
                pass.push(quote!(&#v));
                t.clone()
            }
            ArgKind::Obj(t) => {
                conv.push(quote!(let #v = #p::__private::object::<#t>(#next, cx)?;));
                borrows.push(quote!(let #b = <#t as #p::ExportObject>::__borrow(&#v)?;));
                pass.push(quote!(&*#b));
                t.clone()
            }
            ArgKind::ObjMut(t) => {
                conv.push(quote!(let #v = #p::__private::object::<#t>(#next, cx)?;));
                borrows.push(quote!(let mut #b = #p::__private::borrow_mut::<#t>(&#v)?;));
                pass.push(quote!(&mut *#b));
                t.clone()
            }
        };
        let mut d = quote!(<#dty as #p::JsType>::describe(b));
        if a.bigint {
            d = quote!(#p::__private::bigint(#d));
        }
        params.push(quote!(#p::Param { name: #name, ty: #d }));
    }
    let mut ret_ty: Type = match &sig.output {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, t) => (**t).clone(),
    };
    if let Some(st) = self_ty {
        ReplaceSelf(st).visit_type_mut(&mut ret_ty);
    }
    // E3: a plain fn returning `Stream<T>` hands JS an async iterable at once.
    let stream_ret = last_ident(&ret_ty).as_deref() == Some("Stream");
    if stream_ret {
        if exec.is_some() {
            return Err(syn::Error::new(
                ret_ty.span(),
                "an offloaded export streams through a `Sink<T>` parameter; a `Stream<T>` result is for a plain fn that returns at once",
            ));
        }
        if sinks > 0 {
            return Err(syn::Error::new(ret_ty.span(), "take a `Sink<T>` or return a `Stream<T>`, not both"));
        }
        if js.constructor || js.getter || js.setter {
            return Err(syn::Error::new(ret_ty.span(), "constructors and accessors cannot return a stream"));
        }
        let item = generic_arg(&ret_ty).ok_or_else(|| syn::Error::new(ret_ty.span(), "expected `Stream<T>`"))?;
        items = quote!(#p::__private::item_type::<#item>(b));
    }
    if js.buffer.is_some() && sinks == 0 {
        return Err(syn::Error::new(sig.span(), "#[js(buffer = n)] sizes a `Sink<T>` parameter; a `Stream<T>` result takes its buffer from `channel(n)`"));
    }
    let mut ret_d = quote!(<#ret_ty as #p::JsType>::describe(b));
    let mut throws = quote!(<#ret_ty as #p::JsType>::throws());
    if stream_ret {
        ret_d = quote!(<() as #p::JsType>::describe(b));
        throws = quote!(::std::vec::Vec::new());
    }
    if js.bigint {
        ret_d = quote!(#p::__private::bigint(#ret_d));
    }
    let thunk = format_ident!("__svl_thunk_{idx}");
    let start = format_ident!("__svl_start_{idx}");
    let sigf = format_ident!("__svl_sig_{idx}");
    g.items.push(quote! {
        fn #sigf(b: &mut #p::TypeBuilder) -> #p::Sig {
            #p::Sig { params: ::std::vec![#(#params),*], ret: #ret_d, throws: #throws, progress: #progress, items: #items }
        }
    });

    let offload = match exec {
        None if stream_ret => {
            let (recv_code, recv_arg) = match (recv, self_ty) {
                (Recv::Arc, Some(st)) => (quote!(let __this = #p::__private::this_arc::<#st>(cx)?;), quote!(__this,)),
                (Recv::Mut, Some(st)) => {
                    (quote!(let __this = #p::__private::this::<#st>(cx)?; let mut __s = #p::__private::borrow_mut::<#st>(&__this)?;), quote!(&mut *__s,))
                }
                (Recv::Ref, Some(st)) => {
                    (quote!(let __this = #p::__private::this::<#st>(cx)?; let __s = <#st as #p::ExportObject>::__borrow(&__this)?;), quote!(&*__s,))
                }
                _ => (Ts::new(), Ts::new()),
            };
            g.items.push(quote! {
                #[allow(non_snake_case, unused_mut, unused_variables, clippy::needless_borrow)]
                fn #start(
                    cx: &#p::CallCx<'_>,
                    args: ::std::vec::Vec<#p::Value>,
                    __op: &::std::sync::Arc<#p::__private::OpShared>,
                ) -> ::core::result::Result<#p::__private::Job, #p::NativeError> {
                    let mut __it = args.into_iter();
                    #(#conv)*
                    let __stream: #ret_ty = {
                        #recv_code
                        #(#borrows)*
                        #call_path(#recv_arg #(#pass),*)
                    };
                    ::core::result::Result::Ok(#p::__private::stream_job(__stream, __op))
                }
            });
            quote!(::core::option::Option::Some(#p::OffloadDef { exec: #p::Exec::Local, start: #start, supersede: false, max_in_flight: 0 }))
        }
        None => {
            let (recv_code, recv_arg) = match (recv, self_ty) {
                (Recv::Arc, Some(st)) => (quote!(let __this = #p::__private::this_arc::<#st>(cx)?;), quote!(__this,)),
                (Recv::Mut, Some(st)) => {
                    (quote!(let __this = #p::__private::this::<#st>(cx)?; let mut __s = #p::__private::borrow_mut::<#st>(&__this)?;), quote!(&mut *__s,))
                }
                (Recv::Ref, Some(st)) => {
                    (quote!(let __this = #p::__private::this::<#st>(cx)?; let __s = <#st as #p::ExportObject>::__borrow(&__this)?;), quote!(&*__s,))
                }
                _ => (Ts::new(), Ts::new()),
            };
            g.items.push(quote! {
                #[allow(non_snake_case, unused_mut, unused_variables, clippy::needless_borrow)]
                fn #thunk(cx: &#p::CallCx<'_>, args: ::std::vec::Vec<#p::Value>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                    let mut __it = args.into_iter();
                    #(#conv)*
                    let __ret: #ret_ty = {
                        #recv_code
                        #(#borrows)*
                        #call_path(#recv_arg #(#pass),*)
                    };
                    #p::IntoJs::into_js(__ret, cx)
                }
            });
            quote!(::core::option::Option::None)
        }
        Some(exec) => {
            // The receiver: a clone of the object's holder, taken on the JS thread; the job borrows it.
            let mut recv_take = Ts::new();
            let mut recv_borrow = Ts::new();
            let mut recv_arg = Ts::new();
            if let (true, Some(st)) = (recv != Recv::None, self_ty) {
                if pooled {
                    checks.push(quote_spanned!(st.span()=> #p::__private::assert_offload_self::<#st>();));
                }
                match recv {
                    Recv::Arc => {
                        recv_take = quote!(let __this = #p::__private::this_arc::<#st>(cx)?;);
                        recv_arg = quote!(__this,);
                    }
                    Recv::Ref if pooled => {
                        recv_take = quote!(let __this = #p::__private::this_arc::<#st>(cx)?;);
                        recv_arg = quote!(&*__this,);
                    }
                    Recv::Ref => {
                        recv_take = quote!(let __this = #p::__private::this::<#st>(cx)?;);
                        recv_borrow = quote!(let __s = <#st as #p::ExportObject>::__borrow(&__this)?;);
                        recv_arg = quote!(&*__s,);
                    }
                    Recv::Mut => {
                        recv_take = quote!(let __this = #p::__private::this::<#st>(cx)?;);
                        recv_borrow = quote!(let mut __s = #p::__private::borrow_mut::<#st>(&__this)?;);
                        recv_arg = quote!(&mut *__s,);
                    }
                    Recv::None => {}
                }
            }
            let job = match exec {
                Exec::Worker | Exec::Blocking => {
                    let e = if exec == Exec::Worker { quote!(Worker) } else { quote!(Blocking) };
                    checks.push(quote_spanned!(ret_ty.span()=> #p::__private::assert_worker_send::<#ret_ty>();));
                    quote! {
                        #p::__private::pool_job::<#ret_ty>(#p::Exec::#e, move || -> #ret_ty { #call_path(#recv_arg #(#pass),*) })
                    }
                }
                Exec::Local => quote! {
                    #p::__private::local_job::<#ret_ty>(async move {
                        #recv_borrow
                        let __r: #ret_ty = #call_path(#recv_arg #(#pass),*).await;
                        ::core::result::Result::Ok::<#ret_ty, #p::NativeError>(__r)
                    })
                },
                Exec::Tokio => {
                    checks.push(quote_spanned!(ret_ty.span()=> #p::__private::assert_worker_send::<#ret_ty>();));
                    quote! {
                        #p::__private::tokio_job::<#ret_ty, _>(async move {
                            let __r: #ret_ty = #call_path(#recv_arg #(#pass),*).await;
                            ::core::result::Result::Ok::<#ret_ty, #p::NativeError>(__r)
                        })
                    }
                }
            };
            g.items.push(quote! {
                #[allow(non_snake_case, unused_mut, unused_variables, clippy::needless_borrow)]
                fn #start(
                    cx: &#p::CallCx<'_>,
                    args: ::std::vec::Vec<#p::Value>,
                    __op: &::std::sync::Arc<#p::__private::OpShared>,
                ) -> ::core::result::Result<#p::__private::Job, #p::NativeError> {
                    const _: () = { #(#checks)* };
                    let mut __it = args.into_iter();
                    #(#conv)*
                    #recv_take
                    ::core::result::Result::Ok(#job)
                }
            });
            let e = match exec {
                Exec::Worker => quote!(Worker),
                Exec::Blocking => quote!(Blocking),
                Exec::Local => quote!(Local),
                Exec::Tokio => quote!(Tokio),
            };
            let supersede = js.supersede;
            let max = js.max_in_flight.unwrap_or(0);
            quote!(::core::option::Option::Some(#p::OffloadDef { exec: #p::Exec::#e, start: #start, supersede: #supersede, max_in_flight: #max }))
        }
    };
    let thunk_ref = if exec.is_some() || stream_ret { quote!(#p::__private::offload_thunk) } else { quote!(#thunk) };
    g.defs.push(quote! {
        #p::MemberDef {
            name: #jsname,
            rust: #rust,
            kind: #p::MemberKind::#kind,
            receiver: #p::Receiver::#receiver,
            docs: #fdocs,
            sig: #sigf,
            thunk: #thunk_ref,
            offload: #offload,
        }
    });
    Ok(())
}


/// `#[svelters::export]` on an `impl` block of an exported object: every `pub fn` becomes a member.
/// `#[svelters::export(module = "m")]` on a free function: a function of `svelters:app/m`.
#[proc_macro_attribute]
pub fn export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr: Ts = attr.into();
    let item: Ts = item.into();
    let res = if let Ok(imp) = syn::parse2::<ItemImpl>(item.clone()) {
        export_impl(attr, imp)
    } else if let Ok(f) = syn::parse2::<ItemFn>(item.clone()) {
        export_fn(attr, f)
    } else if let Ok(s) = syn::parse2::<syn::ItemStatic>(item.clone()) {
        export_static(attr, s)
    } else {
        Err(syn::Error::new(Span::call_site(), "#[svelters::export] goes on an `impl` block, a free `fn` or a `static` (an app-wide store)"))
    };
    res.unwrap_or_else(|e| e.to_compile_error()).into()
}

fn export_impl(attr: Ts, mut imp: ItemImpl) -> syn::Result<Ts> {
    if !attr.is_empty() {
        return Err(syn::Error::new(attr.span(), "#[svelters::export] on an impl block takes no arguments (the module comes from #[svelters::object])"));
    }
    if imp.trait_.is_some() {
        return Err(syn::Error::new(imp.span(), "exporting trait impls is not supported (E0)"));
    }
    let self_ty = (*imp.self_ty).clone();
    let rust = quote!(#self_ty).to_string();
    let mut g = Gen { defs: Vec::new(), items: Vec::new() };
    let mut errors: Option<syn::Error> = None;
    for (idx, it) in imp.items.iter_mut().enumerate() {
        let ImplItem::Fn(f) = it else { continue };
        let js = match js_opts(&f.attrs) {
            Ok(j) => j,
            Err(e) => {
                errors.get_or_insert(e.clone()).combine(e);
                continue;
            }
        };
        strip_js(&mut f.attrs);
        if js.skip || !matches!(f.vis, syn::Visibility::Public(_)) {
            // Keep the parameter attributes from reaching rustc.
            for i in f.sig.inputs.iter_mut() {
                if let FnArg::Typed(pt) = i {
                    strip_js(&mut pt.attrs);
                }
            }
            continue;
        }
        let fdocs = docs(&f.attrs);
        let ident = f.sig.ident.clone();
        if let Err(e) = member(&mut g, idx, Some(&self_ty), &mut f.sig, &js, &fdocs, quote!(<#self_ty>::#ident)) {
            match &mut errors {
                Some(x) => x.combine(e),
                None => errors = Some(e),
            }
        }
    }
    if let Some(e) = errors {
        return Err(e);
    }
    let p = quote!(::svelters::__private);
    let items = &g.items;
    let defs = &g.defs;
    Ok(quote! {
        #imp
        const _: () = {
            #(#items)*
            #p::inventory::submit! {
                #p::ImplDef { type_id: || ::std::any::TypeId::of::<#self_ty>(), rust: #rust, members: &[#(#defs),*] }
            }
        };
    })
}

fn export_fn(attr: Ts, mut f: ItemFn) -> syn::Result<Ts> {
    let a = object_args(attr)?;
    let module = a.module.ok_or_else(|| syn::Error::new(Span::call_site(), "#[svelters::export] on a function needs `module = \"...\"`"))?;
    check_module(&module, f.sig.span())?;
    let js = js_opts(&f.attrs)?;
    strip_js(&mut f.attrs);
    if js.constructor || js.getter || js.setter {
        return Err(syn::Error::new(f.sig.span(), "free functions cannot be constructors or accessors"));
    }
    let mut js = js;
    if js.rename.is_none() {
        js.rename = a.name;
    }
    let fdocs = docs(&f.attrs);
    let ident = f.sig.ident.clone();
    let mut g = Gen { defs: Vec::new(), items: Vec::new() };
    member(&mut g, 0, None, &mut f.sig, &js, &fdocs, quote!(#ident))?;
    let p = quote!(::svelters::__private);
    let items = &g.items;
    let def = &g.defs[0];
    Ok(quote! {
        #f
        const _: () = {
            #(#items)*
            #p::inventory::submit! {
                #p::FnDef { module: #module, member: #def }
            }
        };
    })
}

// ---- E3: statics, stores, services, events ----

/// `#[svelters::export(module = "m")]` on a `static` (a `StoreStatic<S>`): the module-level value
/// `export const <camelName>` of `svelters:app/m`, the same Rust object in every JS runtime.
fn export_static(attr: Ts, mut item: syn::ItemStatic) -> syn::Result<Ts> {
    let a = object_args(attr)?;
    let module = a.module.ok_or_else(|| syn::Error::new(Span::call_site(), "#[svelters::export] on a static needs `module = \"...\"`"))?;
    check_module(&module, item.span())?;
    if !matches!(item.mutability, syn::StaticMutability::None) {
        return Err(syn::Error::new(item.span(), "an exported static cannot be `mut`"));
    }
    let js = js_opts(&item.attrs)?;
    strip_js(&mut item.attrs);
    let ident = &item.ident;
    let ty = &item.ty;
    let rust = ident.to_string();
    let jsname = js.rename.or(a.name).unwrap_or_else(|| camel(&rust.to_lowercase()));
    let vdocs = docs(&item.attrs);
    let p = quote!(::svelters::__private);
    Ok(quote! {
        #item
        const _: () = {
            #[allow(non_snake_case)]
            fn __svl_get(cx: &#p::CallCx<'_>, _args: ::std::vec::Vec<#p::Value>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                #p::__private::static_value::<#ty>(&#ident, cx)
            }
            fn __svl_ty(b: &mut #p::TypeBuilder) -> #p::TypeRef {
                #p::__private::static_type::<#ty>(b)
            }
            #p::inventory::submit! {
                #p::__private::ValueDef { module: #module, name: #jsname, rust: #rust, docs: #vdocs, what: "static", ty: __svl_ty, get: __svl_get }
            }
        };
    })
}

struct StoreArgs {
    module: Option<String>,
    name: Option<String>,
}

fn store_args(attr: Ts) -> syn::Result<StoreArgs> {
    let mut a = StoreArgs { module: None, name: None };
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr)?;
    for m in metas {
        match &m {
            Meta::NameValue(nv) if nv.path.is_ident("module") || nv.path.is_ident("name") => {
                let Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value else {
                    return Err(syn::Error::new(nv.value.span(), "expected a string"));
                };
                if nv.path.is_ident("module") {
                    a.module = Some(s.value());
                } else {
                    a.name = Some(s.value());
                }
            }
            _ => return Err(syn::Error::new(m.span(), "expected `module = \"...\"` or `name = \"...\"`")),
        }
    }
    Ok(a)
}

/// `#[store(writable)]`, `#[store(proportional)]` on a store field.
fn store_field_opts(attrs: &[Attribute]) -> syn::Result<(bool, bool)> {
    let (mut writable, mut proportional) = (false, false);
    for a in attrs.iter().filter(|a| a.path().is_ident("store")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("writable") {
                writable = true;
            } else if m.path.is_ident("proportional") {
                proportional = true;
            } else {
                return Err(m.error("expected `writable` or `proportional`"));
            }
            Ok(())
        })?;
    }
    Ok((writable, proportional))
}

/// `#[svelters::store(module = "m")]` on a struct with named fields: reactive state Svelte reads like
/// `$state` (design note §6.4). Each field is a property of the JS store object, `exact` by construction;
/// `#[store(writable)]` fields are assignable from JS (and so bindable), `#[store(proportional)]` fields are
/// delivered as invalidations. Generates `<Name>Field` (an enum naming the fields) and the `StoreState`
/// impl; `Store<Name>` / `StoreStatic<Name>` hold the state. Field types are `Clone + IntoJs`
/// (writable ones `FromJs` too).
#[proc_macro_attribute]
pub fn store(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemStruct);
    store_impl(attr.into(), item).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn store_impl(attr: Ts, mut item: ItemStruct) -> syn::Result<Ts> {
    let a = store_args(attr)?;
    let module = a.module.ok_or_else(|| syn::Error::new(Span::call_site(), "#[svelters::store] needs `module = \"...\"` (the JS module is `svelters:app/<module>`)"))?;
    check_module(&module, item.span())?;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(item.generics.span(), "stores cannot be generic"));
    }
    let Fields::Named(fields) = &mut item.fields else {
        return Err(syn::Error::new(item.span(), "#[svelters::store] needs a struct with named fields"));
    };
    if fields.named.is_empty() {
        return Err(syn::Error::new(item.span(), "a store needs at least one field"));
    }
    let ty = item.ident.clone();
    let rust = ty.to_string();
    let jsname = a.name.unwrap_or_else(|| rust.clone());
    let sdocs = docs(&item.attrs);
    let vis = item.vis.clone();
    let field_enum = format_ident!("{}Field", ty);
    let p = quote!(::svelters::__private);

    let mut variants = Vec::new();
    let mut infos = Vec::new();
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut describes = Vec::new();
    let mut members = Vec::new();
    let mut fns = Vec::new();
    for (i, f) in fields.named.iter_mut().enumerate() {
        let (writable, proportional) = store_field_opts(&f.attrs)?;
        let o = js_opts(&f.attrs)?;
        f.attrs.retain(|a| !a.path().is_ident("store") && !a.path().is_ident("js"));
        if o.bigint {
            return Err(syn::Error::new(f.span(), "`#[js(bigint)]` store fields are not supported (E3): store a checked number (±2^53) or a string"));
        }
        let id = f.ident.clone().expect("named");
        let fty = f.ty.clone();
        let fname = o.rename.clone().unwrap_or_else(|| camel(&id.to_string()));
        let frust = id.to_string();
        let fdocs = docs(&f.attrs);
        let variant = format_ident!("{}", pascal(&frust));
        variants.push(quote!(#variant));
        infos.push(quote!(#p::__private::StoreField { name: #fname, rust: #frust, writable: #writable, proportional: #proportional, docs: #fdocs }));
        reads.push(quote!(#i => #p::IntoJs::into_js(::core::clone::Clone::clone(&self.#id), cx)));
        describes.push(quote!(#i => <#fty as #p::JsType>::describe(b)));
        let get = format_ident!("__svl_get_{i}");
        let sig_get = format_ident!("__svl_sig_get_{i}");
        fns.push(quote! {
            #[allow(non_snake_case)]
            fn #get(cx: &#p::CallCx<'_>, _args: ::std::vec::Vec<#p::Value>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                #p::__private::store_get::<#ty>(cx, #i)
            }
            fn #sig_get(b: &mut #p::TypeBuilder) -> #p::Sig {
                #p::Sig { params: ::std::vec::Vec::new(), ret: <#fty as #p::JsType>::describe(b), throws: ::std::vec::Vec::new(), progress: ::core::option::Option::None, items: ::core::option::Option::None }
            }
        });
        members.push(quote! {
            #p::MemberDef { name: #fname, rust: #frust, kind: #p::MemberKind::Getter, receiver: #p::Receiver::Ref, docs: #fdocs, sig: #sig_get, thunk: #get, offload: ::core::option::Option::None }
        });
        if writable {
            writes.push(quote! {
                #i => {
                    let __new = <#fty as #p::FromJs>::from_js(v, cx)?;
                    let __old = ::std::mem::replace(&mut self.#id, __new);
                    #[allow(unused_imports)]
                    use #p::__private::{HookViaDefault as _, HookViaHooks as _};
                    match (&#p::__private::HookProbe::<Self>::new()).__svl_on_write(self, #field_enum::#variant) {
                        ::core::option::Option::None => ::core::result::Result::Ok(false),
                        ::core::option::Option::Some(::core::result::Result::Ok(())) => ::core::result::Result::Ok(true),
                        ::core::option::Option::Some(::core::result::Result::Err(e)) => {
                            self.#id = __old;
                            ::core::result::Result::Err(e)
                        }
                    }
                }
            });
            let set = format_ident!("__svl_set_{i}");
            let sig_set = format_ident!("__svl_sig_set_{i}");
            fns.push(quote! {
                #[allow(non_snake_case)]
                fn #set(cx: &#p::CallCx<'_>, args: ::std::vec::Vec<#p::Value>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                    #p::__private::store_set::<#ty>(cx, #i, args)
                }
                fn #sig_set(b: &mut #p::TypeBuilder) -> #p::Sig {
                    #p::Sig {
                        params: ::std::vec![#p::Param { name: "value", ty: <#fty as #p::JsType>::describe(b) }],
                        ret: <() as #p::JsType>::describe(b),
                        throws: ::std::vec::Vec::new(),
                        progress: ::core::option::Option::None,
                        items: ::core::option::Option::None,
                    }
                }
            });
            members.push(quote! {
                #p::MemberDef { name: #fname, rust: #frust, kind: #p::MemberKind::Setter, receiver: #p::Receiver::Ref, docs: #fdocs, sig: #sig_set, thunk: #set, offload: ::core::option::Option::None }
            });
            // Writable fields are `FromJs` too (checked here, with the field's span).
            let check = quote_spanned!(fty.span()=> const _: fn() = || { fn __need<T: #p::FromJs>() {} __need::<#fty>(); };);
            fns.push(check);
        }
    }
    let n = variants.len();
    let variant_list = variants.clone();
    Ok(quote! {
        #item

        /// The fields of the store (for `Store::write`, `Store::touch` and `StoreHooks::on_write`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #vis enum #field_enum { #(#variants),* }

        impl #p::StoreState for #ty {
            const MODULE: &'static str = #module;
            const NAME: &'static str = #jsname;
            const DOCS: &'static str = #sdocs;
            const FIELDS: &'static [#p::__private::StoreField] = &[#(#infos),*];
            type Field = #field_enum;
            fn __index(f: #field_enum) -> usize {
                f as usize
            }
            fn __field(i: usize) -> #field_enum {
                const ALL: [#field_enum; #n] = [#(#field_enum::#variant_list),*];
                ALL[i]
            }
            fn __read(&self, i: usize, cx: &#p::CallCx<'_>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                match i {
                    #(#reads,)*
                    _ => ::core::result::Result::Err(#p::host_error("type", ::std::format!("{} has no field #{}", #jsname, i))),
                }
            }
            #[allow(unreachable_code, unused_variables)]
            fn __write(&mut self, i: usize, v: #p::Value, cx: &#p::CallCx<'_>) -> ::core::result::Result<bool, #p::NativeError> {
                match i {
                    #(#writes,)*
                    _ => ::core::result::Result::Err(#p::host_error("type", ::std::format!("{}: field #{} is read-only (#[store(writable)] makes it assignable)", #jsname, i))),
                }
            }
            fn __describe(i: usize, b: &mut #p::TypeBuilder) -> #p::TypeRef {
                match i {
                    #(#describes,)*
                    _ => <() as #p::JsType>::describe(b),
                }
            }
        }

        const _: () = {
            #(#fns)*
            #p::inventory::submit! {
                #p::ObjectDef { module: #module, name: #jsname, rust: #rust, docs: #sdocs, shared: true, type_id: || ::std::any::TypeId::of::<#p::__private::StoreCell<#ty>>() }
            }
            #p::inventory::submit! {
                #p::ImplDef { type_id: || ::std::any::TypeId::of::<#p::__private::StoreCell<#ty>>(), rust: #rust, members: &[#(#members),*] }
            }
            #p::inventory::submit! {
                #p::__private::StoreDef {
                    type_id: || ::std::any::TypeId::of::<#p::__private::StoreCell<#ty>>(),
                    fields: <#ty as #p::StoreState>::FIELDS,
                    subscribe: #p::__private::subscribe_any::<#ty>,
                }
            }
        };
    })
}

/// `#[svelters::service(module = "m")]` on `fn name(cx: ServiceCx) -> V`: a service that starts with the
/// app (or, with `lazy`, when JS first imports `svelters:app/m`), runs until `shutdown_services`, and is
/// never restarted by a hot reload. JS imports its value `V` (a `Store<S>` or a shared object) as
/// `export const <name>`.
#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemFn);
    service_impl(attr.into(), item).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn service_impl(attr: Ts, mut f: ItemFn) -> syn::Result<Ts> {
    let (mut module, mut name, mut lazy) = (None, None, false);
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr)?;
    for m in metas {
        match &m {
            Meta::Path(p) if p.is_ident("lazy") => lazy = true,
            Meta::NameValue(nv) if nv.path.is_ident("module") || nv.path.is_ident("name") => {
                let Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value else {
                    return Err(syn::Error::new(nv.value.span(), "expected a string"));
                };
                if nv.path.is_ident("module") {
                    module = Some(s.value());
                } else {
                    name = Some(s.value());
                }
            }
            _ => return Err(syn::Error::new(m.span(), "expected `module = \"...\"`, `name = \"...\"` or `lazy`")),
        }
    }
    let module = module.ok_or_else(|| syn::Error::new(Span::call_site(), "#[svelters::service] needs `module = \"...\"`"))?;
    check_module(&module, f.sig.span())?;
    let js = js_opts(&f.attrs)?;
    strip_js(&mut f.attrs);
    if f.sig.asyncness.is_some() {
        return Err(syn::Error::new(f.sig.span(), "a service fn is synchronous: start long-running work with `cx.thread(...)` (or `cx.spawn(...)` with the `tokio` feature)"));
    }
    if !f.sig.generics.params.is_empty() || f.sig.inputs.len() != 1 {
        return Err(syn::Error::new(f.sig.span(), "a service is `fn name(cx: ServiceCx) -> V`"));
    }
    let ReturnType::Type(_, ret) = &f.sig.output else {
        return Err(syn::Error::new(f.sig.span(), "a service returns what JS imports: a `Store<S>` or a shared object (`Arc<T>`)"));
    };
    let ret = (**ret).clone();
    let ident = f.sig.ident.clone();
    let rust = ident.to_string();
    let jsname = js.rename.or(name).unwrap_or_else(|| camel(&rust));
    let sdocs = docs(&f.attrs);
    let p = quote!(::svelters::__private);
    Ok(quote! {
        #f
        const _: () = {
            static __SVL_SLOT: #p::__private::ServiceSlot<#ret> = #p::__private::ServiceSlot::new();
            fn __svl_start() {
                let _ = __SVL_SLOT.get_or_start(#module, #jsname, #ident);
            }
            #[allow(non_snake_case)]
            fn __svl_get(cx: &#p::CallCx<'_>, _args: ::std::vec::Vec<#p::Value>) -> ::core::result::Result<#p::Value, #p::NativeError> {
                let v = __SVL_SLOT.get_or_start(#module, #jsname, #ident);
                #p::IntoJs::into_js(::core::clone::Clone::clone(v), cx)
            }
            fn __svl_ty(b: &mut #p::TypeBuilder) -> #p::TypeRef {
                <#ret as #p::JsType>::describe(b)
            }
            #p::inventory::submit! {
                #p::__private::ServiceDef { module: #module, name: #jsname, lazy: #lazy, start: __svl_start }
            }
            #p::inventory::submit! {
                #p::__private::ValueDef { module: #module, name: #jsname, rust: #rust, docs: #sdocs, what: "service", ty: __svl_ty, get: __svl_get }
            }
        };
    })
}

/// `#[derive(svelters::Events)]` on an enum: one JS event per variant (`obj.on('<kebab-name>', fn)`). A unit
/// variant has no argument, a one-field tuple variant passes its value, a struct variant passes a plain
/// object (camelCase keys). `#[event(latest)]`: only the last occurrence per checkpoint is delivered.
/// Needs `Clone` (an event goes to every listening host) and `Send`.
#[proc_macro_derive(Events, attributes(js, event))]
pub fn derive_events(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    events(input).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn events(input: DeriveInput) -> syn::Result<Ts> {
    let name = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(input.generics.span(), "event enums cannot be generic"));
    }
    let Data::Enum(de) = &input.data else {
        return Err(syn::Error::new(input.span(), "#[derive(svelters::Events)] needs an enum"));
    };
    if de.variants.len() > 64 {
        return Err(syn::Error::new(input.span(), "an event enum has at most 64 variants"));
    }
    let p = quote!(::svelters::__private);
    let mut infos = Vec::new();
    let mut index = Vec::new();
    let mut payload = Vec::new();
    let mut describe = Vec::new();
    for (i, v) in de.variants.iter().enumerate() {
        let o = js_opts(&v.attrs)?;
        let mut latest = false;
        for a in v.attrs.iter().filter(|a| a.path().is_ident("event")) {
            a.parse_nested_meta(|m| {
                if m.path.is_ident("latest") {
                    latest = true;
                    Ok(())
                } else {
                    Err(m.error("expected `latest`"))
                }
            })?;
        }
        let ename = o.rename.unwrap_or_else(|| kebab(&v.ident.to_string()));
        let vdocs = docs(&v.attrs);
        infos.push(quote!(#p::__private::EventInfo { name: #ename, latest: #latest, docs: #vdocs }));
        let id = &v.ident;
        match &v.fields {
            Fields::Unit => {
                index.push(quote!(#name::#id => #i));
                payload.push(quote!(#name::#id => ::core::result::Result::Ok(::core::option::Option::None)));
                describe.push(quote!(::core::option::Option::None));
            }
            Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                let t = &u.unnamed[0].ty;
                index.push(quote!(#name::#id(..) => #i));
                payload.push(quote!(#name::#id(__x) => ::core::result::Result::Ok(::core::option::Option::Some(#p::IntoJs::into_js(__x, cx)?))));
                describe.push(quote!(::core::option::Option::Some(<#t as #p::JsType>::describe(b))));
            }
            Fields::Unnamed(u) => {
                return Err(syn::Error::new(u.span(), "an event passes one value: use one field, or named fields (a plain object)"));
            }
            Fields::Named(nf) => {
                let ids: Vec<_> = nf.named.iter().map(|f| f.ident.clone().expect("named")).collect();
                let mut keys = Vec::new();
                let mut tys = Vec::new();
                for f in &nf.named {
                    let fo = js_opts(&f.attrs)?;
                    keys.push(fo.rename.unwrap_or_else(|| camel(&f.ident.as_ref().expect("named").to_string())));
                    tys.push(f.ty.clone());
                }
                index.push(quote!(#name::#id { .. } => #i));
                payload.push(quote! {
                    #name::#id { #(#ids),* } => ::core::result::Result::Ok(::core::option::Option::Some(#p::Value::Map(::std::vec![
                        #((::std::string::String::from(#keys), #p::IntoJs::into_js(#ids, cx)?)),*
                    ])))
                });
                describe.push(quote! {{
                    let mut __ts = ::std::string::String::from("{ ");
                    #(
                        __ts.push_str(&::std::format!("{}: {}; ", #keys, <#tys as #p::JsType>::describe(b).ts));
                    )*
                    __ts.push('}');
                    ::core::option::Option::Some(#p::__private::object_type(__ts))
                }});
            }
        }
    }
    Ok(quote! {
        impl #p::EventSet for #name {
            const EVENTS: &'static [#p::__private::EventInfo] = &[#(#infos),*];
            fn __index(&self) -> usize {
                match self { #(#index),* }
            }
            fn __payload(self, cx: &#p::CallCx<'_>) -> ::core::result::Result<::core::option::Option<#p::Value>, #p::NativeError> {
                match self { #(#payload),* }
            }
            fn __describe(b: &mut #p::TypeBuilder) -> ::std::vec::Vec<::core::option::Option<#p::TypeRef>> {
                ::std::vec![#(#describe),*]
            }
        }
    })
}

// ---- #[svelters::widget] (RA1, ARCH-EMBED §8.2) ----

/// A provider kind for the egui backend: a struct whose `#[prop]` fields are set from Svelte, whose
/// `#[event] x: Event<T>` fields are its `on<x>` events, and whose `#[state]` (or unmarked) fields are
/// Rust-only node state. The struct implements `svelters_egui::provider::Widget` itself.
///
/// ```ignore
/// #[svelters::widget(namespace = "cad", tag = "quantity-field", caps = FOCUSABLE | EDITABLE)]
/// pub struct QuantityField {
///     /// The committed value.
///     #[prop(bind)] value: Option<f64>,
///     #[prop(default = true)] units_shown: bool,
///     #[event] accepted: Event<f64>,
///     #[state] text: String,
/// }
/// ```
///
/// - `namespace` (required): the kind's spec id is `<namespace>.<StructName>`, and the props' named
///   types are declared in `svelters:app/<namespace>`.
/// - `tag`: the element (default: the struct name in kebab-case). `caps`: `svelters_egui::provider::Caps`
///   names joined with `|`.
/// - `#[prop(bind)]`: the widget writes it with `cx.set` (`bind:` observes it, `exact`);
///   `#[prop(default = expr)]`; `#[prop(measure)]`: a change measures the node again;
///   `#[prop(rename = "x")]`. JS names are the field names in kebab-case. A prop type is any
///   `FromJs + IntoJs + Clone` (`IntoJs` for reads), so records, enums, `Option`, `Vec`, `Json<T>`.
/// - Props without `default` and state fields start at `Default::default()`.
///
/// Own widgets (OW0, ARCH-WIDGETS §1.3):
/// - `#[behavior] press: Pressable` (or `#[behavior(params = method)]`, a `fn(&self) -> Params`):
///   a behaviour unit (`svelters_egui::interact::Unit`) run in declaration order by the generated
///   `self.run_units(cx, ui, &resp)`, which returns a `<Name>Fired` struct with one field of
///   intents per unit; also generated: `units_sense()`, `units_a11y(node)`,
///   `units_claims(key, mods)` and `units_next_frame()`.
/// - `#[part] mark: Part` (a paint part) and `#[part(node)] thumb: Part` (a node part): the
///   `::part(name)` names of the kind, in kebab-case.
/// - `replaces = "<tag>"` (svelters-egui only): the struct implements that built-in kind; the tag
///   is the built-in's, and props and events are routed to the kind's row by name. Props and
///   events the row lacks become extras of the kind (`#[prop(enumeration = "a|b")]` for an enum).
/// - `style_as = "<tag>"`: the kind is also matched by the built-in's type selector (the theme
///   styles `cad-slider` as a `slider`; parts of the same names get the built-in's part rules).
///
/// The generated code names `::svelters_egui`, so the crate depends on `svelters-egui` too.
///
/// RA3: `#[svelters::widget]` on an `impl` block of a widget struct gives the kind methods JS calls
/// on the element (`bind:this`): every fn with a `&self`/`&mut self` receiver (pub or not; fns
/// without a receiver stay Rust-only), named in lowerCamelCase (`#[method(rename = "x")]` or
/// `#[js(rename = "x")]`; `#[method(skip)]`/`#[js(skip)]`). Parameters and results are any
/// `FromJs`/`IntoJs` type (`Json<T>` for serde values); a `Result<T, E>` with a `#[derive(svelters::Error)]`
/// `E` throws the error in JS; a `&mut MethodCx` parameter is the call context.
///
/// ```ignore
/// #[svelters::widget]
/// impl CodeEditor {
///     /// Scrolls line `n` into view.
///     pub fn reveal_line(&mut self, n: u32) { … }
///     pub fn focus(&mut self, cx: &mut MethodCx<'_>) { cx.request_focus() }
/// }
/// ```
#[proc_macro_attribute]
pub fn widget(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr: Ts = attr.into();
    let item: Ts = item.into();
    let res = if let Ok(s) = syn::parse2::<ItemStruct>(item.clone()) {
        widget_impl(attr, s, false)
    } else if let Ok(imp) = syn::parse2::<ItemImpl>(item.clone()) {
        widget_methods(attr, imp)
    } else if let Ok(f) = syn::parse2::<ItemFn>(item.clone()) {
        widget_fn(attr, f)
    } else {
        Err(syn::Error::new(Span::call_site(), "#[svelters::widget] goes on a struct (a kind), an `impl` block of one (its methods) or a fn (the fn form)"))
    };
    res.unwrap_or_else(|e| e.to_compile_error()).into()
}

/// RA3 (egui-rust-exports §7.3): an ecosystem egui widget as a kind, with little glue. A struct
/// form kind (`#[widget(..)]` takes `#[svelters::widget]`'s arguments) plus a closure
/// `|p: &mut Self, ui: &mut egui::Ui| -> egui::Response` that shows the widget from the props.
/// `#[prop(bind)]` fields the widget writes in place are diffed after the call and announced
/// (`cx.set`), so `bind:` is exact; the size is measured by running the closure in a sizing pass.
///
/// ```ignore
/// svelters::wrap_widget! {
///     /// `<drag-value bind:value={x} speed={0.1} />`
///     #[widget(namespace = "demo", tag = "drag-value")]
///     pub struct DragValueKind {
///         #[prop(bind)] value: f64,
///         #[prop(default = 1.0)] speed: f64,
///         #[prop] suffix: String,
///     }
///     |p, ui| ui.add(egui::DragValue::new(&mut p.value).speed(p.speed).suffix(p.suffix.clone()))
/// }
/// ```
#[proc_macro]
pub fn wrap_widget(input: TokenStream) -> TokenStream {
    wrap_widget_impl(input.into()).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn wrap_widget_impl(input: Ts) -> syn::Result<Ts> {
    let parser = |s: syn::parse::ParseStream| -> syn::Result<(ItemStruct, Expr)> {
        let item: ItemStruct = s.parse()?;
        let body: Expr = s.parse()?;
        let _: Option<Token![;]> = s.parse()?;
        Ok((item, body))
    };
    let (mut item, body) = parser.parse2(input)?;
    let pos = item
        .attrs
        .iter()
        .position(|a| a.path().is_ident("widget"))
        .ok_or_else(|| syn::Error::new(item.ident.span(), "wrap_widget! needs `#[widget(namespace = \"…\", tag = \"…\")]` on the struct"))?;
    let attr = item.attrs.remove(pos);
    let args = match attr.meta {
        Meta::List(l) => l.tokens,
        _ => return Err(syn::Error::new(attr.span(), "`#[widget(namespace = \"…\")]`")),
    };
    if !matches!(body, Expr::Closure(_)) {
        return Err(syn::Error::new(body.span(), "wrap_widget!: after the struct, a closure `|p, ui| ui.add(…)` that returns the widget's `egui::Response`"));
    }
    let binds = bind_fields(&item)?;
    let name = item.ident.clone();
    let kind = widget_impl(args, item, false)?;
    let shown = wrapped_widget(&name, &binds, quote! {
        let __show: fn(&mut #name, &mut ::svelters_egui::egui::Ui) -> ::svelters_egui::egui::Response = #body;
        __show(p, ui)
    });
    Ok(quote! { #kind #shown })
}

/// Whether a `#[prop(..)]` attribute has `bind`.
fn prop_has_bind(a: &Attribute) -> syn::Result<bool> {
    let mut bind = false;
    if matches!(a.meta, Meta::List(_)) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("bind") {
                bind = true;
            } else if m.input.peek(Token![=]) {
                let _: Expr = m.value()?.parse()?;
            }
            Ok(())
        })?;
    }
    Ok(bind)
}

/// The `#[prop(..bind..)]` fields of a struct (what a wrapped widget may write in place).
fn bind_fields(item: &ItemStruct) -> syn::Result<Vec<syn::Ident>> {
    let mut out = Vec::new();
    for f in item.fields.iter() {
        for a in f.attrs.iter().filter(|a| a.path().is_ident("prop")) {
            if prop_has_bind(a)? {
                out.push(f.ident.clone().expect("named field"));
            }
        }
    }
    Ok(out)
}

/// `impl Widget` for a kind drawn by `show` (statements over `p: &mut Self` and `ui`, giving the
/// response): `ui` runs it and announces the bound fields it changed through `cx.set`; `measure`
/// runs it in a sizing pass with the bound fields restored afterwards.
fn wrapped_widget(name: &syn::Ident, binds: &[syn::Ident], show: Ts) -> Ts {
    let olds: Vec<syn::Ident> = (0..binds.len()).map(|i| format_ident!("__old{i}")).collect();
    quote! {
        const _: () = {
            fn __svl_show(p: &mut #name, ui: &mut ::svelters_egui::egui::Ui) -> ::svelters_egui::egui::Response {
                #show
            }

            impl ::svelters_egui::provider::Widget for #name {
                fn measure(&mut self, cx: &mut ::svelters_egui::provider::MeasureCx<'_>, avail: ::svelters_egui::provider::AvailableSpace) -> ::svelters_egui::egui::Vec2 {
                    #(let #olds = ::core::clone::Clone::clone(&self.#binds);)*
                    let __size = cx.sizing_pass(avail, |ui| {
                        let _ = __svl_show(self, ui);
                    });
                    #(self.#binds = #olds;)*
                    __size
                }

                fn ui(&mut self, ui: &mut ::svelters_egui::egui::Ui, cx: &mut ::svelters_egui::provider::WidgetCx<'_>) -> ::svelters_egui::egui::Response {
                    #(let #olds = ::core::clone::Clone::clone(&self.#binds);)*
                    let __r = __svl_show(self, ui);
                    #(
                        let __new = ::core::mem::replace(&mut self.#binds, #olds);
                        cx.set(&mut self.#binds, __new);
                    )*
                    __r
                }
            }
        };
    }
}

/// RA3: `#[svelters::widget(namespace = "…")] fn name(ui: &mut egui::Ui, props…) -> egui::Response`,
/// the fn form for stateless widgets: a struct `Name` (the fn name in PascalCase) with one prop per
/// parameter after the `Ui` (`#[prop(default = e, rename = "x", measure)]` as on fields). A `T`
/// parameter gets a clone of the prop, `&T` (`&str` for a `String` prop) borrows it, and
/// `&mut T` is `#[prop(bind)]`: what the fn writes is announced (`cx.set`). The fn stays callable
/// from Rust.
fn widget_fn(attr: Ts, mut f: ItemFn) -> syn::Result<Ts> {
    if !f.sig.generics.params.is_empty() || f.sig.asyncness.is_some() {
        return Err(syn::Error::new(f.sig.span(), "the fn form is a plain, non-generic fn"));
    }
    let fname = f.sig.ident.clone();
    let name = format_ident!("{}", pascal(&fname.to_string()), span = fname.span());
    let mut fields = Vec::new();
    let mut pass = Vec::new();
    let mut binds = Vec::new();
    let mut saw_ui = false;
    for (i, input) in f.sig.inputs.iter_mut().enumerate() {
        let FnArg::Typed(pt) = input else {
            return Err(syn::Error::new(input.span(), "the fn form is a free fn (no receiver)"));
        };
        let prop_attrs: Vec<Attribute> = pt.attrs.iter().filter(|a| a.path().is_ident("prop")).cloned().collect();
        pt.attrs.retain(|a| !a.path().is_ident("prop"));
        if i == 0 {
            let is_ui = matches!(&*pt.ty, Type::Reference(r) if r.mutability.is_some() && last_ident(&r.elem).as_deref() == Some("Ui"));
            if !is_ui || !prop_attrs.is_empty() {
                return Err(syn::Error::new(pt.ty.span(), "the fn form's first parameter is `ui: &mut egui::Ui`"));
            }
            saw_ui = true;
            pass.push(quote!(ui));
            continue;
        }
        let ident = match &*pt.pat {
            Pat::Ident(pi) => pi.ident.clone(),
            _ => return Err(syn::Error::new(pt.pat.span(), "a prop parameter is a plain name (it becomes the prop's name)")),
        };
        let mut has_bind = false;
        for a in &prop_attrs {
            has_bind |= prop_has_bind(a)?;
        }
        let (fty, arg, bind): (Type, Ts, bool) = match &*pt.ty {
            Type::Reference(r) if r.mutability.is_some() => ((*r.elem).clone(), quote!(&mut p.#ident), true),
            Type::Reference(r) => {
                let t: Type = match &*r.elem {
                    Type::Path(tp) if tp.path.is_ident("str") => syn::parse_quote!(::std::string::String),
                    Type::Slice(s) => {
                        let e = &s.elem;
                        syn::parse_quote!(::std::vec::Vec<#e>)
                    }
                    t => t.clone(),
                };
                (t, quote!(&p.#ident), false)
            }
            t => (t.clone(), quote!(::core::clone::Clone::clone(&p.#ident)), false),
        };
        if bind != has_bind {
            return Err(syn::Error::new(
                pt.ty.span(),
                if bind { "a `&mut T` parameter is a bound prop: mark it #[prop(bind)]" } else { "#[prop(bind)] takes a `&mut T` parameter (the fn writes it)" },
            ));
        }
        if bind {
            binds.push(ident.clone());
        }
        let attrs = if prop_attrs.is_empty() { vec![syn::parse_quote!(#[prop])] } else { prop_attrs };
        fields.push(quote! { #(#attrs)* #ident: #fty });
        pass.push(arg);
    }
    if !saw_ui {
        return Err(syn::Error::new(f.sig.span(), "the fn form's first parameter is `ui: &mut egui::Ui`"));
    }
    let vis = &f.vis;
    let doc_attrs: Vec<&Attribute> = f.attrs.iter().filter(|a| a.path().is_ident("doc")).collect();
    let item: ItemStruct = syn::parse_quote! {
        #(#doc_attrs)*
        #vis struct #name { #(#fields),* }
    };
    let kind = widget_impl(attr, item, false)?;
    let shown = wrapped_widget(&name, &binds, quote! { #fname(#(#pass),*) });
    Ok(quote! {
        #f
        #kind
        #shown
    })
}


/// Names a JS method of a provider kind cannot have: the runtime node's own members (`bind:this`
/// gives the node; a method of the same name would only be reachable through `call`).
const RESERVED_METHODS: &[&str] = &[
    "alive", "animate", "attached", "attrs", "call", "captured", "cattrs", "constructor", "dead", "destroyed", "dispatchEvent",
    "getAnimations", "getBoundingClientRect", "getProp", "h", "handle", "handlers", "host", "inert", "kind", "last", "listening",
    "nested", "np", "owner", "passive", "sc", "scope", "setProp", "shown", "slotName", "slots", "style", "svlHandle", "tag", "then",
    "tokens", "top", "toString", "valueOf",
];

/// RA3: `#[svelters::widget] impl T { pub fn … }`, the methods of provider kind `T`.
fn widget_methods(attr: Ts, mut imp: ItemImpl) -> syn::Result<Ts> {
    if !attr.is_empty() {
        return Err(syn::Error::new(attr.span(), "#[svelters::widget] on an impl block takes no arguments (the kind comes from the struct)"));
    }
    if imp.trait_.is_some() {
        return Err(syn::Error::new(imp.span(), "#[svelters::widget] goes on an inherent impl block (`impl MyKind { … }`), not a trait impl"));
    }
    if !imp.generics.params.is_empty() {
        return Err(syn::Error::new(imp.generics.span(), "a kind's methods cannot be generic"));
    }
    let self_ty = (*imp.self_ty).clone();
    let mut entries = Vec::new();
    let mut arms = Vec::new();
    let mut describe = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut errors: Option<syn::Error> = None;
    fn push_err(e: syn::Error, errors: &mut Option<syn::Error>) {
        match errors {
            Some(x) => x.combine(e),
            None => *errors = Some(e),
        }
    }
    for it in imp.items.iter_mut() {
        let ImplItem::Fn(f) = it else { continue };
        let mut js = match js_opts(&f.attrs) {
            Ok(j) => j,
            Err(e) => {
                push_err(e, &mut errors);
                continue;
            }
        };
        strip_js(&mut f.attrs);
        // `#[method]`, `#[method(rename = "x")]`, `#[method(skip)]` (cadatomic K1's spelling; `#[js(..)]` too)
        for a in f.attrs.iter().filter(|a| a.path().is_ident("method")) {
            if matches!(a.meta, Meta::List(_)) {
                if let Err(e) = a.parse_nested_meta(|m| {
                    if m.path.is_ident("rename") {
                        js.rename = Some(m.value()?.parse::<syn::LitStr>()?.value());
                    } else if m.path.is_ident("skip") {
                        js.skip = true;
                    } else {
                        return Err(m.error("#[method] options: rename = \"name\", skip"));
                    }
                    Ok(())
                }) {
                    push_err(e, &mut errors);
                }
            }
        }
        f.attrs.retain(|a| !a.path().is_ident("method"));
        // every fn with a receiver is a method; associated fns (no receiver) are Rust-only
        let has_receiver = matches!(f.sig.inputs.first(), Some(FnArg::Receiver(_)));
        if js.skip || !has_receiver {
            continue;
        }
        if js.constructor || js.getter || js.setter || js.is_static || js.worker || js.blocking || js.local || js.tokio || js.bigint || js.code.is_some() {
            push_err(syn::Error::new(f.sig.span(), "a kind method takes only #[js(rename = \"x\")] or #[js(skip)]"), &mut errors);
            continue;
        }
        let sig = &f.sig;
        if sig.asyncness.is_some() || !sig.generics.params.is_empty() {
            push_err(syn::Error::new(sig.span(), "a kind method is a plain, non-generic fn (it runs between passes on the UI thread)"), &mut errors);
            continue;
        }
        let rust = sig.ident.to_string();
        let jsname = js.rename.clone().unwrap_or_else(|| camel(rust.trim_start_matches("r#")));
        if RESERVED_METHODS.contains(&jsname.as_str()) {
            push_err(
                syn::Error::new(sig.ident.span(), format!("`{jsname}` is a member of every element node in JS; rename the method (#[js(rename = \"…\")])")),
                &mut errors,
            );
            continue;
        }
        if names.contains(&jsname) {
            push_err(syn::Error::new(sig.ident.span(), format!("two methods named `{jsname}`")), &mut errors);
            continue;
        }
        let mut recv_mut = None;
        let mut conv = Vec::new();
        let mut pass = Vec::new();
        let mut params = Vec::new();
        let mut types = Vec::new();
        let mut bad = false;
        for (i, input) in sig.inputs.iter().enumerate() {
            match input {
                FnArg::Receiver(r) => {
                    if r.reference.is_none() || r.colon_token.is_some() {
                        push_err(syn::Error::new(r.span(), "a kind method takes `&self` or `&mut self` (the node's widget)"), &mut errors);
                        bad = true;
                    }
                    recv_mut = Some(r.mutability.is_some());
                }
                FnArg::Typed(pt) => {
                    let ty = &*pt.ty;
                    if let Type::Reference(r) = ty {
                        if r.mutability.is_some() && last_ident(&r.elem).as_deref() == Some("MethodCx") {
                            pass.push(quote!(&mut *mcx));
                            continue;
                        }
                        push_err(syn::Error::new(ty.span(), "a kind method's parameters are owned values (`String`, `Vec<T>`, records); only `&mut MethodCx` is borrowed"), &mut errors);
                        bad = true;
                        continue;
                    }
                    let name = match &*pt.pat {
                        Pat::Ident(pi) => camel(pi.ident.to_string().trim_start_matches('_')),
                        _ => format!("arg{i}"),
                    };
                    let v = format_ident!("__a{i}");
                    conv.push(quote! {
                        let #v = <#ty as __p::FromJs>::from_js(__it.next().unwrap_or(__p::Value::Null), cx).map_err(|e| __p::at(e, #name))?;
                    });
                    pass.push(quote!(#v));
                    types.push(quote!(b.ty::<#ty>()));
                    params.push(name);
                }
            }
        }
        if bad {
            continue;
        }
        if recv_mut.is_none() {
            push_err(syn::Error::new(sig.span(), "a kind method needs a receiver (`&self` or `&mut self`); associated fns are not methods (make it private or #[js(skip)])"), &mut errors);
            continue;
        }
        let ret: Type = match &sig.output {
            ReturnType::Default => syn::parse_quote!(()),
            ReturnType::Type(_, t) => (**t).clone(),
        };
        let ident = &sig.ident;
        let fdocs = docs(&f.attrs);
        let idx = entries.len();
        entries.push(quote! {
            ::svelters_egui::provider::MethodEntry { name: #jsname, rust: #rust, docs: #fdocs, params: &[#(#params),*] }
        });
        arms.push(quote! {
            #idx => {
                let mut __it = args.into_iter();
                #(#conv)*
                let __this = ::svelters_egui::provider::__this::<#self_ty>(w, <#self_ty as ::svelters_egui::provider::KindDecl>::KIND)?;
                let __r: #ret = <#self_ty>::#ident(__this, #(#pass),*);
                __p::IntoJs::into_js(__r, cx)
            }
        });
        describe.push(quote! { #(v.push(#types);)* v.push(b.ty::<#ret>()); });
        names.push(jsname);
    }
    if let Some(e) = errors {
        return Err(e);
    }
    Ok(quote! {
        #imp

        const _: () = {
            use ::svelters_egui::provider::__private as __p;

            static __SVL_METHODS: &[::svelters_egui::provider::MethodEntry] = &[#(#entries),*];

            #[allow(unreachable_code, unused_variables, unused_mut, clippy::needless_borrow)]
            fn __svl_call(
                w: &mut dyn ::core::any::Any,
                m: usize,
                args: ::std::vec::Vec<__p::Value>,
                mcx: &mut ::svelters_egui::provider::MethodCx<'_>,
                cx: &__p::CallCx<'_>,
            ) -> ::core::result::Result<__p::Value, __p::NativeError> {
                match m {
                    #(#arms)*
                    _ => ::core::result::Result::Err(__p::no_member("method", m)),
                }
            }

            #[allow(unused_mut)]
            fn __svl_method_types(b: &mut __p::TypeBuilder) -> ::std::vec::Vec<__p::TypeRef> {
                let mut v = ::std::vec::Vec::new();
                #(#describe)*
                v
            }

            __p::inventory::submit! {
                ::svelters_egui::provider::KindMethods {
                    kind: ::svelters_egui::provider::__kind_of::<#self_ty>,
                    methods: __SVL_METHODS,
                    call: __svl_call,
                }
            }
            __p::inventory::submit! {
                __p::KindTypesDef {
                    module: <#self_ty as ::svelters_egui::provider::KindDecl>::__NAMESPACE,
                    kind: <#self_ty as ::svelters_egui::provider::KindDecl>::__METHODS_KEY,
                    describe: __svl_method_types,
                }
            }
        };
    })
}

// ---- #[svelters::behavior] (EV2b, ARCH-EMBED §8.5) ----

/// A native behaviour for the egui backend, used from Svelte as `use:<name>`: an attached kind whose
/// `#[prop]` fields are the action's params and whose `#[event] x: Event<T>` fields are the params'
/// `on<x>` callbacks. The struct implements `svelters_egui::provider::Behavior` itself (`after_ui`
/// runs after the host's `ui` every frame, in Rust).
///
/// ```ignore
/// #[svelters::behavior(namespace = "cad", name = "pick")]
/// pub struct Pick {
///     /// The value kinds this host presents.
///     #[prop] accepts: Vec<String>,
///     #[event] picked: Event<PickRef>,
///     #[state] hovered: bool,
/// }
/// ```
///
/// - `namespace` (required): the spec id is `<namespace>.<StructName>`, the action is exported by
///   `svelters:behaviors/<namespace>`, and the params' named types are declared in
///   `svelters:app/<namespace>`.
/// - `name`: the JS export (default: the struct name in lowerCamelCase); the node's kind tag is
///   `<namespace>.<name>`, which no template can write.
/// - Fields as in `#[svelters::widget]` (`#[prop(default = expr, rename = "x")]`, `#[event]`,
///   `#[state]`), without `bind` (no `bind:` through an action: report values with an event) or
///   `measure`.
#[proc_macro_attribute]
pub fn behavior(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemStruct);
    widget_impl(attr.into(), item, true)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// OW0: the kind-table type of a prop or event payload of an own widget (what an extra prop of a
/// replaced built-in kind is in the compiler's spec): from the Rust type, or `enumeration`.
fn row_ty(ty: &Type, enumeration: &Option<Vec<String>>) -> (Ts, bool) {
    if let Some(values) = enumeration {
        return (quote! { ::svelters_egui::provider::RowTy::Enum(&[#(#values),*]) }, false);
    }
    let (inner, nullable) = match last_segment(ty) {
        Some(s) if s.ident == "Option" => (first_generic(ty).unwrap_or(ty), true),
        _ => (ty, false),
    };
    let t = match last_segment(inner).map(|s| s.ident.to_string()).as_deref() {
        Some("bool") => quote! { Bool },
        Some("i64" | "i32" | "u32" | "u16" | "u8" | "i16" | "i8" | "usize" | "isize" | "u64") => quote! { I64 },
        Some("f64" | "f32") => quote! { F64 },
        Some("String" | "str") => quote! { Str },
        _ => quote! { Other },
    };
    (quote! { ::svelters_egui::provider::RowTy::#t }, nullable)
}

enum WidgetField {
    Prop { name: String, bind: bool, measure: bool, default: Option<Expr>, enumeration: Option<Vec<String>> },
    Event,
    State,
    /// OW0: a behaviour unit, with the method giving its params.
    Behavior { params: Option<syn::Ident> },
    /// OW0: a part, `node` for a node part.
    Part { name: String, node: bool },
}

fn caps_expr(e: &Expr, out: &mut Vec<syn::Ident>) -> syn::Result<()> {
    match e {
        Expr::Binary(b) if matches!(b.op, syn::BinOp::BitOr(_)) => {
            caps_expr(&b.left, out)?;
            caps_expr(&b.right, out)
        }
        Expr::Path(p) if p.path.get_ident().is_some() => {
            out.push(p.path.get_ident().unwrap().clone());
            Ok(())
        }
        Expr::Paren(p) => caps_expr(&p.expr, out),
        other => Err(syn::Error::new(other.span(), "caps: capability names joined with `|` (FOCUSABLE | EDITABLE)")),
    }
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(p) => p.path.segments.last(),
        _ => None,
    }
}

fn first_generic(ty: &Type) -> Option<&Type> {
    match &last_segment(ty)?.arguments {
        syn::PathArguments::AngleBracketed(a) => a.args.iter().find_map(|g| match g {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        }),
        _ => None,
    }
}

fn widget_impl(attr: Ts, mut item: ItemStruct, behavior: bool) -> syn::Result<Ts> {
    let what = if behavior {
        "#[svelters::behavior]"
    } else {
        "#[svelters::widget]"
    };
    let mut namespace: Option<String> = None;
    let mut tag: Option<String> = None;
    let mut bname: Option<String> = None;
    let mut caps: Vec<syn::Ident> = Vec::new();
    let mut replaces: Option<String> = None;
    let mut style_as: Option<String> = None;
    syn::meta::parser(|m| {
        if m.path.is_ident("namespace") {
            namespace = Some(m.value()?.parse::<syn::LitStr>()?.value());
        } else if behavior {
            if m.path.is_ident("name") {
                bname = Some(m.value()?.parse::<syn::LitStr>()?.value());
            } else {
                return Err(m.error("expected `namespace = \"...\"` or `name = \"...\"`"));
            }
        } else if m.path.is_ident("tag") {
            tag = Some(m.value()?.parse::<syn::LitStr>()?.value());
        } else if m.path.is_ident("caps") {
            let e: Expr = m.value()?.parse()?;
            caps_expr(&e, &mut caps)?;
        } else if m.path.is_ident("replaces") {
            replaces = Some(m.value()?.parse::<syn::LitStr>()?.value());
        } else if m.path.is_ident("style_as") {
            style_as = Some(m.value()?.parse::<syn::LitStr>()?.value());
        } else {
            return Err(m.error("expected `namespace = \"...\"`, `tag = \"...\"`, `caps = A | B` or `replaces = \"<tag>\"`"));
        }
        Ok(())
    })
    .parse2(attr)?;
    let name = item.ident.clone();
    let namespace = namespace.ok_or_else(|| {
        syn::Error::new(name.span(), format!("{what} needs `namespace = \"...\"`"))
    })?;
    if namespace.is_empty()
        || !namespace
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(syn::Error::new(
            name.span(),
            "the namespace must be a plain name (`cad`)",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            format!("a {what} struct cannot be generic"),
        ));
    }
    let tag = if behavior {
        let n = bname.unwrap_or_else(|| {
            let s = name.to_string();
            let mut c = s.chars();
            c.next()
                .map(|f| f.to_ascii_lowercase().to_string() + c.as_str())
                .unwrap_or_default()
        });
        if n.is_empty()
            || n.starts_with(|c: char| c.is_ascii_digit())
            || !n
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        {
            return Err(syn::Error::new(
                name.span(),
                format!(
                    "behaviour name `{n}`: a JS identifier (it is the export of svelters:behaviors/{namespace})"
                ),
            ));
        }
        format!("{namespace}.{n}")
    } else {
        tag.or_else(|| replaces.clone()).unwrap_or_else(|| kebab(&name.to_string()))
    };
    if let Some(r) = &replaces
        && *r != tag
    {
        return Err(syn::Error::new(name.span(), format!("`replaces = \"{r}\"`: the tag is the replaced built-in's (`{r}`), not `{tag}`")));
    }
    if !behavior && replaces.is_none() && (!tag.contains('-') || tag.chars().any(|c| c.is_ascii_uppercase())) {
        return Err(syn::Error::new(
            name.span(),
            format!(
                "tag `{tag}`: a provider tag is lower-case kebab-case with a dash (`quantity-field`); capitalised tags are Svelte components"
            ),
        ));
    }
    let id = format!("{namespace}.{name}");
    let methods_key = format!("{id}#methods");
    let kind_docs = docs(&item.attrs);
    let Fields::Named(fields) = &mut item.fields else {
        return Err(syn::Error::new(
            item.span(),
            format!("{what} needs a struct with named fields"),
        ));
    };

    let mut kinds: Vec<(syn::Ident, Type, String, WidgetField)> = Vec::new();
    for f in fields.named.iter_mut() {
        let ident = f.ident.clone().expect("named field");
        let fdocs = docs(&f.attrs);
        let mut kind = WidgetField::State;
        let mut keep = Vec::new();
        for a in f.attrs.drain(..) {
            if a.path().is_ident("prop") {
                let mut bind = false;
                let mut measure = false;
                let mut default = None;
                let mut rename = None;
                let mut enumeration = None;
                if matches!(a.meta, Meta::List(_)) {
                    a.parse_nested_meta(|m| {
                        if m.path.is_ident("enumeration") {
                            let v = m.value()?.parse::<syn::LitStr>()?.value();
                            enumeration = Some(v.split('|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect());
                        } else if m.path.is_ident("bind") && !behavior {
                            bind = true;
                        } else if m.path.is_ident("measure") && !behavior {
                            measure = true;
                        } else if m.path.is_ident("default") {
                            default = Some(m.value()?.parse::<Expr>()?);
                        } else if m.path.is_ident("rename") {
                            rename = Some(m.value()?.parse::<syn::LitStr>()?.value());
                        } else {
                            return Err(m.error("#[prop] options: bind, measure, default = expr, rename = \"name\", enumeration = \"a|b\""));
                        }
                        Ok(())
                    })?;
                }
                let js = rename.unwrap_or_else(|| ident.to_string().trim_start_matches("r#").replace('_', "-"));
                kind = WidgetField::Prop { name: js, bind, measure, default, enumeration };
            } else if a.path().is_ident("event") {
                if last_segment(&f.ty).is_none_or(|s| s.ident != "Event") {
                    return Err(syn::Error::new(f.ty.span(), "an #[event] field has type `Event<T>`"));
                }
                kind = WidgetField::Event;
            } else if a.path().is_ident("state") {
                kind = WidgetField::State;
            } else if a.path().is_ident("behavior") && !behavior {
                let mut params = None;
                if matches!(a.meta, Meta::List(_)) {
                    a.parse_nested_meta(|m| {
                        if m.path.is_ident("params") {
                            params = Some(m.value()?.parse::<syn::Ident>()?);
                            Ok(())
                        } else {
                            Err(m.error("#[behavior] options: params = method"))
                        }
                    })?;
                }
                kind = WidgetField::Behavior { params };
            } else if a.path().is_ident("part") && !behavior {
                let mut node = false;
                let mut pname: Option<String> = None;
                if matches!(a.meta, Meta::List(_)) {
                    a.parse_nested_meta(|m| {
                        if m.path.is_ident("node") {
                            node = true;
                            Ok(())
                        } else if m.path.is_ident("paint") {
                            Ok(())
                        } else if m.path.is_ident("name") {
                            pname = Some(m.value()?.parse::<syn::LitStr>()?.value());
                            Ok(())
                        } else {
                            Err(m.error("#[part] options: node, paint, name = \"part-name\""))
                        }
                    })?;
                }
                let js = pname.unwrap_or_else(|| ident.to_string().trim_start_matches("r#").replace('_', "-"));
                kind = WidgetField::Part { name: js, node };
                // a paint part is only a name: its field is never read
                keep.push(syn::parse_quote!(#[allow(dead_code)]));
            } else {
                keep.push(a);
            }
        }
        f.attrs = keep;
        kinds.push((ident, f.ty.clone(), fdocs, kind));
    }

    let caps_tokens = if caps.is_empty() {
        quote! { ::svelters_egui::provider::Caps::NONE }
    } else {
        quote! { ::svelters_egui::provider::Caps(0 #(| ::svelters_egui::provider::Caps::#caps.0)*) }
    };
    let mut prop_entries = Vec::new();
    let mut event_entries = Vec::new();
    let mut describe = Vec::new();
    let mut event_describe = Vec::new();
    let mut inits = Vec::new();
    let mut sets = Vec::new();
    let mut resets = Vec::new();
    let mut gets = Vec::new();
    let mut prop_i = 0usize;
    let mut event_i = 0u16;
    // OW0: behaviour units and parts
    let mut part_entries = Vec::new();
    let mut unit_fields: Vec<(syn::Ident, Type, Ts)> = Vec::new();
    for (ident, ty, fdocs, kind) in &kinds {
        match kind {
            WidgetField::Prop { name: js, bind, measure, default, enumeration } => {
                let optional = last_segment(ty).is_some_and(|s| s.ident == "Option");
                let (rty, nullable) = row_ty(ty, enumeration);
                // RA3: a bound prop gets its value from `bind:`, so it is never required
                let required = default.is_none() && !optional && !*bind;
                let init = match default {
                    Some(d) => quote! { ::core::convert::Into::into(#d) },
                    None => quote! { ::core::default::Default::default() },
                };
                prop_entries.push(quote! {
                    ::svelters_egui::provider::PropEntry {
                        name: #js, docs: #fdocs, bind: #bind, required: #required, measure: #measure,
                        offset: ::core::mem::offset_of!(#name, #ident), row_ty: #rty, nullable: #nullable,
                    }
                });
                describe.push(quote! { b.ty::<#ty>() });
                inits.push(quote! { #ident: #init });
                let i = prop_i;
                sets.push(quote! {
                    #i => { self.#ident = <#ty as __p::FromJs>::from_js(v, cx).map_err(|e| __p::at(e, #js))?; ::core::result::Result::Ok(()) }
                });
                resets.push(quote! { #i => self.#ident = #init, });
                gets.push(quote! { #i => __p::IntoJs::into_js(::core::clone::Clone::clone(&self.#ident), cx), });
                prop_i += 1;
            }
            WidgetField::Event => {
                let js = ident.to_string().trim_start_matches("r#").replace('_', "-");
                let payload = first_generic(ty).ok_or_else(|| syn::Error::new(ty.span(), "an #[event] field has type `Event<T>`"))?;
                let arg = match payload {
                    Type::Tuple(t) if t.elems.is_empty() => quote! { ::core::option::Option::None },
                    other => {
                        let (t, _) = row_ty(other, &None);
                        quote! { ::core::option::Option::Some(#t) }
                    }
                };
                event_entries.push(quote! { ::svelters_egui::provider::EventEntry { name: #js, docs: #fdocs, arg: #arg } });
                event_describe.push(quote! { b.ty::<#payload>() });
                let i = event_i;
                inits.push(quote! { #ident: ::svelters_egui::provider::Event::__new(#i) });
                event_i += 1;
            }
            WidgetField::State => inits.push(quote! { #ident: ::core::default::Default::default() }),
            WidgetField::Behavior { params } => {
                inits.push(quote! { #ident: ::core::default::Default::default() });
                let p = match params {
                    Some(m) => quote! { self.#m() },
                    None => quote! { <<#ty as ::svelters_egui::interact::Unit>::Params as ::core::default::Default>::default() },
                };
                unit_fields.push((ident.clone(), ty.clone(), p));
            }
            WidgetField::Part { name: js, node } => {
                inits.push(quote! { #ident: ::core::default::Default::default() });
                part_entries.push(quote! { ::svelters_egui::provider::PartEntry { name: #js, docs: #fdocs, node: #node } });
            }
        }
    }
    let vis = &item.vis;
    let units_impl = if unit_fields.is_empty() {
        quote! {}
    } else {
        let fired = syn::Ident::new(&format!("{name}Fired"), name.span());
        let idents: Vec<&syn::Ident> = unit_fields.iter().map(|(i, _, _)| i).collect();
        let tys: Vec<&Type> = unit_fields.iter().map(|(_, t, _)| t).collect();
        let params: Vec<&Ts> = unit_fields.iter().map(|(_, _, p)| p).collect();
        let pnames: Vec<syn::Ident> = idents.iter().map(|i| syn::Ident::new(&format!("__p_{i}"), i.span())).collect();
        let fired_doc = format!("The intents of [`{name}`]'s behaviour units in one pass (`run_units`), one field per unit.");
        quote! {
            #[doc = #fired_doc]
            #[allow(dead_code)]
            #vis struct #fired {
                #( pub #idents: ::svelters_egui::interact::Intents<<#tys as ::svelters_egui::interact::Unit>::Intent>, )*
            }

            #[allow(dead_code)]
            impl #name {
                /// What the units sense, merged (pass it to `cx.interact`).
                #vis fn units_sense(&self) -> ::svelters_egui::egui::Sense {
                    #( let #pnames = #params; )*
                    ::svelters_egui::egui::Sense::hover() #( | ::svelters_egui::interact::Unit::sense(&self.#idents, &#pnames) )*
                }

                /// Runs the units in declaration order on `resp` (the node's `cx.interact`), sets
                /// their state bits and restyles in the same frame; returns their intents.
                #vis fn run_units(&mut self, cx: &mut ::svelters_egui::provider::WidgetCx<'_>, ui: &::svelters_egui::egui::Ui, resp: &::svelters_egui::egui::Response) -> #fired {
                    #( let #pnames = #params; )*
                    let mut __fired = #fired { #( #idents: ::core::default::Default::default(), )* };
                    {
                        let __f = &mut __fired;
                        #( let #idents = &mut self.#idents; )*
                        cx.run_units(ui, resp, |__icx| {
                            #( ::svelters_egui::interact::Unit::input(#idents, &#pnames, __icx, &mut __f.#idents); )*
                        });
                    }
                    if let ::core::option::Option::Some(__d) = self.units_next_frame() {
                        ui.ctx().request_repaint_after(__d);
                    }
                    __fired
                }

                /// The units' part of the node's AccessKit node.
                #vis fn units_a11y(&self, node: &mut ::svelters_egui::egui::accesskit::Node) {
                    #( ::svelters_egui::interact::Unit::a11y(&self.#idents, &#params, node); )*
                }

                /// EV3 R11: whether a unit claims `key` while the node is focused.
                #vis fn units_claims(&self, key: ::svelters_egui::egui::Key, m: ::svelters_egui::egui::Modifiers) -> bool {
                    false #( || ::svelters_egui::interact::Unit::claims(&self.#idents, &#params, key, m) )*
                }

                /// The soonest frame a unit needs without input.
                #vis fn units_next_frame(&self) -> ::core::option::Option<::std::time::Duration> {
                    [#( ::svelters_egui::interact::Unit::next_frame(&self.#idents) ),*].into_iter().flatten().min()
                }
            }
        }
    };
    let replaces_tokens = match &replaces {
        Some(r) => quote! { ::core::option::Option::Some(#r) },
        None => quote! { ::core::option::Option::None },
    };
    let style_as_tokens = match &style_as {
        Some(r) => quote! { ::core::option::Option::Some(#r) },
        None => quote! { ::core::option::Option::None },
    };
    // an own widget replacing a built-in takes the row's types: no export-schema module of its own
    let types_submit = if replaces.is_some() {
        quote! {}
    } else {
        quote! { __p::inventory::submit! { __p::KindTypesDef { module: #namespace, kind: #id, describe: __svl_types } } }
    };
    let name_str = name.to_string();
    let (new_body, attaches_to) = if behavior {
        (
            quote! { ::svelters_egui::provider::__new_behavior::<#name>() },
            quote! { ::core::option::Option::Some("Widget") },
        )
    } else {
        (
            quote! { ::std::boxed::Box::new(<#name as ::svelters_egui::provider::KindDecl>::__svelters_new()) },
            quote! { ::core::option::Option::None },
        )
    };
    Ok(quote! {
        #item

        const _: () = {
            use ::svelters_egui::provider::__private as __p;

            static __SVL_PROPS: &[::svelters_egui::provider::PropEntry] = &[#(#prop_entries),*];
            static __SVL_EVENTS: &[::svelters_egui::provider::EventEntry] = &[#(#event_entries),*];
            static __SVL_PARTS: &[::svelters_egui::provider::PartEntry] = &[#(#part_entries),*];

            fn __svl_new() -> ::std::boxed::Box<dyn ::svelters_egui::provider::ErasedWidget> {
                #new_body
            }

            #[allow(dead_code)]
            fn __svl_types(b: &mut __p::TypeBuilder) -> ::std::vec::Vec<__p::TypeRef> {
                ::std::vec![#(#describe,)* #(#event_describe),*]
            }

            static __SVL_KIND: ::svelters_egui::provider::KindEntry = ::svelters_egui::provider::KindEntry {
                namespace: #namespace,
                name: #name_str,
                id: #id,
                tag: #tag,
                rust: ::core::concat!(::core::module_path!(), "::", #name_str),
                docs: #kind_docs,
                caps: #caps_tokens,
                props: __SVL_PROPS,
                events: __SVL_EVENTS,
                new: __svl_new,
                attaches_to: #attaches_to,
                replaces: #replaces_tokens,
                style_as: #style_as_tokens,
                parts: __SVL_PARTS,
            };

            __p::inventory::submit! { ::svelters_egui::provider::KindRef(&__SVL_KIND) }
            #types_submit

            impl ::svelters_egui::provider::KindDecl for #name {
                const KIND: &'static ::svelters_egui::provider::KindEntry = &__SVL_KIND;
                const __NAMESPACE: &'static str = #namespace;
                const __METHODS_KEY: &'static str = #methods_key;

                fn __svelters_new() -> Self {
                    #name { #(#inits),* }
                }

                #[allow(unreachable_code, unused_variables)]
                fn __svelters_set(&mut self, prop: usize, v: __p::Value, cx: &__p::CallCx<'_>) -> ::core::result::Result<(), __p::NativeError> {
                    match prop {
                        #(#sets)*
                        _ => ::core::result::Result::Err(__p::no_member("prop", prop)),
                    }
                }

                fn __svelters_reset(&mut self, prop: usize) {
                    match prop {
                        #(#resets)*
                        _ => {}
                    }
                }

                #[allow(unreachable_code, unused_variables)]
                fn __svelters_get(&self, prop: usize, cx: &__p::CallCx<'_>) -> ::core::result::Result<__p::Value, __p::NativeError> {
                    match prop {
                        #(#gets)*
                        _ => ::core::result::Result::Err(__p::no_member("prop", prop)),
                    }
                }
            }
        };

        #units_impl
    })
}

/// AU1b: `#[svelters_automation::test(app = "...", seed = 3, iterations = 20, clock = "virtual",
/// step_ms = 16)]` on `fn name(d: &mut E2e) -> TestResult`: a `#[test]` that runs the body once per
/// seed on a fresh app, in process (kittest) or, with `SVELTERS_E2E=process`, out of process
/// (`svelters-run`). See `svelters_automation::e2e`. `app` is relative to the test crate's
/// `CARGO_MANIFEST_DIR`; the other arguments are optional (seed 0, 1 iteration, virtual clock,
/// 16 ms frames). Generated code names `::svelters_automation::e2e::*`.
#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemFn);
    test_impl(attr.into(), item).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn test_impl(attr: Ts, item: ItemFn) -> syn::Result<Ts> {
    let (mut app, mut seed, mut iterations, mut clock, mut step_ms) = (None::<syn::LitStr>, None::<Expr>, None::<Expr>, None::<syn::LitStr>, None::<Expr>);
    let parser = syn::meta::parser(|m| {
        let key = m.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
        match key.as_str() {
            "app" => app = Some(m.value()?.parse()?),
            "seed" => seed = Some(m.value()?.parse()?),
            "iterations" => iterations = Some(m.value()?.parse()?),
            "clock" => {
                let c: syn::LitStr = m.value()?.parse()?;
                if c.value() != "virtual" && c.value() != "real" {
                    return Err(syn::Error::new(c.span(), "clock: \"virtual\" or \"real\""));
                }
                clock = Some(c);
            }
            "step_ms" => step_ms = Some(m.value()?.parse()?),
            _ => return Err(m.error("svelters::test: expected app, seed, iterations, clock or step_ms")),
        }
        Ok(())
    });
    parser.parse2(attr)?;
    let app = app.ok_or_else(|| syn::Error::new(Span::call_site(), "svelters::test: `app = \"<dir or App.svelte>\"` is required"))?;
    if item.sig.inputs.len() != 1 || item.sig.asyncness.is_some() {
        return Err(syn::Error::new(item.sig.span(), "svelters::test: the body is `fn name(d: &mut E2e) -> TestResult`"));
    }
    let ItemFn { attrs, vis, sig, block } = item;
    let name = &sig.ident;
    let name_s = name.to_string();
    let mut inner = sig.clone();
    inner.ident = format_ident!("__svelters_test_body");
    let mut sets = Vec::new();
    if let Some(s) = seed {
        sets.push(quote! { spec.seed = (#s) as u64; });
    }
    if let Some(n) = iterations {
        sets.push(quote! { spec.iterations = (#n) as u64; });
    }
    if let Some(c) = clock {
        sets.push(quote! { spec.clock = #c; });
    }
    if let Some(ms) = step_ms {
        sets.push(quote! { spec.step_ms = (#ms) as f64; });
    }
    // The built-in `#[test]` by path: a `use svelters_automation::test;` would shadow the bare name.
    Ok(quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        #vis fn #name() {
            #inner #block
            #[allow(unused_mut)]
            let mut spec = ::svelters_automation::e2e::TestSpec::new(#name_s, #app, env!("CARGO_MANIFEST_DIR"));
            #(#sets)*
            spec.run_bin = option_env!("CARGO_BIN_EXE_svelters-run");
            ::svelters_automation::e2e::run(spec, __svelters_test_body);
        }
    })
}
