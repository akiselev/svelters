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
                "buffer" => return Err(m.error("#[js(buffer)] belongs to streams (Sink<T>), which arrive with E3")),
                "tag" => return Err(m.error("data enums (#[js(tag)]) are not supported yet (E0): use a record with an enum field")),
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
        return Err(syn::Error::new(input.span(), "#[derive(svelters::Record)] needs a struct with named fields (data enums are not supported in E0)"));
    };
    let Fields::Named(fields) = &ds.fields else {
        return Err(syn::Error::new(ds.fields.span(), "#[derive(svelters::Record)] needs named fields"));
    };
    let p = quote!(::svelters::__private);
    let jsname = js_opts(&input.attrs)?.rename.unwrap_or_else(|| name.to_string());
    let tdocs = docs(&input.attrs);
    let n = fields.named.len();
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
        froms.push(match &o.default {
            Some(Some(e)) => quote!(#id: #p::__private::or_default::<#ty>(__f.next().unwrap_or(#p::Value::Null), cx, || #e)?),
            Some(None) => quote!(#id: #p::__private::or_default::<#ty>(__f.next().unwrap_or(#p::Value::Null), cx, ::core::default::Default::default)?),
            None => quote!(#id: <#ty as #p::FromJs>::from_js(__f.next().unwrap_or(#p::Value::Null), cx)?),
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
                let mut __f = #p::__private::record_fields(v, #n)?;
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
/// names (`#[js(rename)]` per variant).
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
    let jsname = js_opts(&input.attrs)?.rename.unwrap_or_else(|| name.to_string());
    let tdocs = docs(&input.attrs);
    let mut values = Vec::new();
    let mut from = Vec::new();
    let mut into = Vec::new();
    for (i, v) in de.variants.iter().enumerate() {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new(v.span(), "#[derive(svelters::Enum)] variants must be unit variants"));
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
                ::core::result::Result::Ok(match #p::__private::enum_index(v)? {
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
    Ok(quote! {
        #item
        #flavor
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
                                    return Err(syn::Error::new(r.span(), "`&mut [T]` needs buffers (E2); take `Vec<T>` and return it"));
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
            ArgKind::Value(t) => {
                conv.push(quote!(let #v = <#t as #p::FromJs>::from_js(#next, cx)?;));
                pass.push(quote!(#v));
                if pooled {
                    checks.push(quote_spanned!(t.span()=> #p::__private::assert_worker_send::<#t>();));
                }
                t.clone()
            }
            ArgKind::ByRef(t) => {
                conv.push(quote!(let #v = <#t as #p::FromJs>::from_js(#next, cx)?;));
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
    let mut ret_d = quote!(<#ret_ty as #p::JsType>::describe(b));
    if js.bigint {
        ret_d = quote!(#p::__private::bigint(#ret_d));
    }
    let thunk = format_ident!("__svl_thunk_{idx}");
    let start = format_ident!("__svl_start_{idx}");
    let sigf = format_ident!("__svl_sig_{idx}");
    g.items.push(quote! {
        fn #sigf(b: &mut #p::TypeBuilder) -> #p::Sig {
            #p::Sig { params: ::std::vec![#(#params),*], ret: #ret_d, throws: <#ret_ty as #p::JsType>::throws(), progress: #progress }
        }
    });

    let offload = match exec {
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
    let thunk_ref = if exec.is_some() { quote!(#p::__private::offload_thunk) } else { quote!(#thunk) };
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
    } else {
        Err(syn::Error::new(Span::call_site(), "#[svelters::export] goes on an `impl` block or a free `fn`"))
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
