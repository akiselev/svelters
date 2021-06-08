use std::collections::{HashMap, HashSet};

use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{self as s, parse_str as p};

use crate::{constants::*, types::*};

pub fn codegen<'a>(classes: &HashMap<String, Class>) -> Result<String> {
    let output: HashSet<&str> = classes
        .values()
        .flat_map(|cls| {
            let mut output = vec![cls.def.name.as_str()];
            output.extend(cls.inherits.iter().map(|c| c.as_str()));
            output
        })
        .filter(|c| !c.contains("<"))
        .filter(|c| !c.contains("::"))
        .collect();
    let parents: HashMap<String, Vec<&str>> = classes
        .values()
        .map(|cls| {
            (
                cls.def.name.clone(),
                cls.inherits
                    .iter()
                    .filter(|c| !c.contains("::"))
                    .filter(|c| !c.contains("<"))
                    .filter(|c| **c != "IQObject")
                    .map(|c| c.as_ref())
                    .collect(),
            )
        })
        .collect();
    let children: HashSet<_> = classes
        .values()
        .flat_map(|cls| {
            let mut output = vec![cls.def.name.clone()];
            output.extend(
                cls.inherits
                    .iter()
                    .map(|c| c.clone())
                    .filter(|c| *c != "QObject")
                    .collect::<Vec<_>>(),
            );
            output
        })
        .collect();
    let children: Vec<_> = children
        .iter()
        .map(|c| ty(format!("I{}", c)).unwrap())
        .collect();
    std::fs::write("./test.json", serde_json::to_string(&parents).unwrap()).unwrap();

    let mut code = vec![];
    for name in output.into_iter() {
        let cls = if let Some(cls) = classes.get(name) {
            cls
        } else {
            continue;
        };
        let interface = codegen_interface(
            cls.def.name.as_str(),
            cls.inherits.get(0).map(|c| c.as_str()),
        )?;
        let obj_impl = codegen_root(cls.def.name.as_str(), &children)?;
        let constructor =
            if !cls.is_abstract() && !CODEGEN_IGNORE_CONSTRUCTOR.contains(&cls.def.name.as_str()) {
                codegen_constructor(cls.def.name.as_str())?
            } else {
                quote! {}
            };
        let types = codegen_types(cls)?;
        code.push(quote! {
            #types
            #obj_impl
            #interface
            #constructor
        });
    }
    for (name, parents) in parents.into_iter() {
        let cls = if let Some(cls) = classes.get(&name) {
            cls
        } else {
            panic!("Could not find key {}", &name);
        };

        for parent in &parents {
            if *parent == "QObject" {
                continue;
            }
            let interface = codegen_cast(cls.def.name.as_str(), &parent)?;
            code.push(interface);
        }
    }
    let obj_impl = codegen_root("QObject", &children)?;
    let widget_int = codegen_interface("QWidget", Some("QObject"))?;
    let widget_impl = codegen_root("QWidget", &children)?;
    let widget_cast = codegen_cast("QWidget", "QWidget")?;
    let code = code.iter();
    let output = quote! {
        #[dyn_cast(#(#children),*)]
        #[dyn_upcast]
        pub trait IQObject {
            unsafe fn as_qt_ref(&self) -> Option<QObject>;
        }

        #obj_impl
        #widget_int
        #widget_impl
        #widget_cast

        #(#code)*
    };
    Ok(format!(
        "use cast_trait_object::{{dyn_cast, dyn_upcast, DynCastExt}};\nuse cpp::{{cpp, cpp_class}};\nuse qttypes::*;\nuse crate::{{QObject, QWidget,QObjectPtr, QWidgetPtr, QRef}};\n\n{}\n",
        codegen_headers(
            classes
                .values()
                .filter(|c| !c.def.name.contains("<"))
                .filter(|c| !c.def.name.contains("::"))
        )?
    ) + output.to_string().as_str())
}

fn codegen_root<'a>(name: &str, children: &Vec<s::Type>) -> Result<TokenStream> {
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

fn codegen_cast<'a>(name: &str, dest: &str) -> Result<TokenStream> {
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

fn codegen_constructor<'a>(name: &str) -> Result<TokenStream> {
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

fn codegen_interface<'a>(name: &str, supertrait: Option<&str>) -> Result<TokenStream> {
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

fn codegen_types<'a>(cls: &Class) -> Result<TokenStream> {
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

fn codegen_headers<'a>(classes: impl Iterator<Item = &'a Class>) -> Result<String> {
    let mut classes = classes
        .map(|cls| format!("#include <{}/{}>", cls.namespace(), cls.def.name))
        .collect::<Vec<_>>();
    classes.push("#include <memory>".to_string());
    classes.push("#include <tuple>".to_string());
    classes.push("#include <QtCore/QPointer>".to_string());
    classes.push("#include <QtCore/QObject>".to_string());
    Ok(format!("cpp!{{{{\n{}\n}}}}\n", classes.join("\n")))
}

fn id<T: AsRef<str>>(name: T) -> Result<s::Ident> {
    Ok(p::<s::Ident>(name.as_ref())?)
}

fn ty<T: AsRef<str>>(name: T) -> Result<s::Type> {
    Ok(p::<s::Type>(name.as_ref())?)
}

fn litstr<T: AsRef<str>>(name: T) -> Result<s::Lit> {
    Ok(p::<s::Lit>(&format!("\"{}\"", name.as_ref()))?)
}
