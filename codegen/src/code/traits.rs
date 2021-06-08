use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{self as s};

use crate::{helpers::*, types::*};

pub(crate) fn codegen_root<'a>(name: &str, children: &Vec<s::Type>) -> Result<TokenStream> {
    let trait_name = ty("IQObject")?;
    let ptr_name = ty(&format!("{}Ptr", name))?;
    let self_type = litstr(&format!("QPointer<{}>*", name))?;
    let return_type = litstr("std::tuple<bool, QObject*>")?;
    let dest = ty("QObject")?;

    let code = quote! {
        #[dyn_cast(#(#children),*)]
        #[dyn_upcast]
        impl #trait_name for #ptr_name {
            unsafe fn as_qt_ref(&self) -> Option<#dest> {
                let (is_safe, ptr) = cpp!([self as #self_type] -> (bool, #dest) as #return_type {
                    if (self->isNull()) {
                        return std::make_tuple(false, (#dest*) NULL);
                    } else {
                        return std::make_tuple(true, qobject_cast<#dest*>(*self));
                    }
                });
                match is_safe {
                    true => Some(ptr),
                    false => None,
                }
            }
        }
    };
    Ok(code)
}

pub(crate) fn codegen_cast<'a>(name: &str, dest: &str) -> Result<TokenStream> {
    let trait_name = ty(&format!("I{}", dest))?;
    let ptr_name = ty(&format!("{}Ptr", name))?;
    let self_type = litstr(&format!("QPointer<{}>*", name))?;
    let return_type = litstr(&format!("std::tuple<bool, {}*>", dest))?;

    let dest = ty(&dest)?;
    let code = quote! {
        impl #trait_name for #ptr_name {
            fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, #dest>> {
                unsafe {
                    let (is_safe, ptr) = cpp!([self as #self_type] -> (bool, #dest) as #return_type {
                        if (self->isNull()) {
                            return std::make_tuple(false, (#dest*) NULL);
                        } else {
                            return std::make_tuple(true, qobject_cast<#dest*>(*self));
                        }
                    });
                    match is_safe {
                        true => Some(QRef::new::<'a>(ptr)),
                        false => None,
                    }
                }
            }
        }
    };
    Ok(code)
}

pub(crate) fn codegen_constructor<'a>(name: &str) -> Result<TokenStream> {
    let _type = ty(&format!("{}", name))?;
    let ptr_type = ty(&format!("{}Ptr", name))?;
    let return_type = litstr(&format!("std::tuple<bool, QPointer<{}>>", name))?;

    let code = quote! {
        impl #_type {
            pub fn new() -> Option<#ptr_type> {
                unsafe {
                    let (is_safe, ptr) = cpp!([] -> (bool, #ptr_type) as #return_type {
                        QPointer<#_type> btn = new #_type();
                        return std::make_tuple(!btn.isNull(), btn);
                    });
                    if is_safe { Some(ptr) }  else { None }
                }
            }
        }
    };
    Ok(code)
}

pub(crate) fn codegen_interface<'a>(name: &str, supertrait: Option<&str>) -> Result<TokenStream> {
    let trait_name = ty(&format!("I{}", name))?;
    let wrapper_name = ty(name)?;
    let supertrait = if let Some(supertrait) = supertrait.map(|s| ty(format!("I{}", s)).unwrap()) {
        quote! {: #supertrait}
    } else {
        quote! {}
    };

    let code = quote! {
        pub trait #trait_name #supertrait {
            fn as_qt_ref<'a>(&'a self) -> Option<QRef<'a, #wrapper_name>>;
        }
    };
    Ok(code)
}

pub(crate) fn codegen_types<'a>(cls: &Class) -> Result<TokenStream> {
    let ptr_name = ty(&format!("{}Ptr", cls.def.name))?;
    let wrapper_name = ty(&format!("{}", cls.def.name))?;

    let ptr_type = litstr(&format!("QPointer<{}>", cls.def.name))?;
    let wrapper_type = litstr(&format!("{}*", cls.def.name))?;

    let code = quote! {
        cpp_class!(
            pub unsafe struct #ptr_name as #ptr_type
        );

        cpp_class!(
            pub unsafe struct #wrapper_name as #wrapper_type
        );
    };
    Ok(code)
}

pub(crate) fn codegen_headers<'a>(classes: impl Iterator<Item = &'a Class>) -> Result<String> {
    let mut classes = classes
        .map(|cls| format!("#include <{}/{}>", cls.namespace(), cls.def.name))
        .collect::<Vec<_>>();
    classes.push("#include <memory>".to_string());
    classes.push("#include <tuple>".to_string());
    classes.push("#include <QtCore/QPointer>".to_string());
    classes.push("#include <QtCore/QObject>".to_string());
    Ok(format!("cpp!{{{{\n{}\n}}}}\n", classes.join("\n")))
}
