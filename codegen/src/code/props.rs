use std::collections::{HashMap, HashSet};

use anyhow::Result;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{self as s, parse_str as p};

use crate::{constants::*, helpers::*, types::*};

pub(crate) fn codegen_props(class: &Class) -> Result<TokenStream> {
    let mut output: Vec<TokenStream> = vec![];
    let name = ty(&class.def.name)?;
    let mut names = HashSet::<String>::new();
    VisitProp::new(|prop| {
        if names.contains(&prop.setter.name) {
            // TODO: Handle overloads instead of bailing out immediately
            return;
        }
        if !IMPLEMENTED_TYPES.contains(&prop._type.as_str())
            && !PRIMITIVE_TYPES.contains(&prop._type.as_str())
        {
            println!("NOT SUPPORTED: {}", &prop._type.as_str());
            return;
        }
        names.insert(prop.setter.name.clone());
        let getter = getter(class, prop).unwrap();
        let setter = setter(class, prop).unwrap();

        output.push(quote! {
            #getter
            #setter
        });
    })
    .on_class(class);
    let code = if output.len() > 0 {
        quote! {
            impl<'a> QRef<'a, #name> {
                #(#output)*
            }
        }
    } else {
        quote! {}
    };
    Ok(code)
}

fn getter(class: &Class, prop: &Property) -> Result<TokenStream> {
    let name = id(&prop.getter.name).expect(&prop.getter.name);
    let _type = ty(&prop._type).expect(&prop._type);
    let self_type = litstr(&format!("{}*", class.def.name)).expect(&prop.getter.name);
    let return_type = litstr(&prop._type)?;

    let code = if IMPLEMENTED_TYPES.contains(&prop._type.as_str()) {
        quote! {
            pub fn #name(self) -> #_type {
                cpp!(unsafe [self as #self_type] -> #_type as #return_type {
                    return #_type(self->#name());
                })
            }
        }
    } else {
        quote! {
               pub fn #name(self) -> #_type {
                   cpp!(unsafe [self as #self_type] -> #_type as #return_type {
                       return self->#name();
                   })
               }
        }
    };
    Ok(code)
}

fn setter(class: &Class, prop: &Property) -> Result<TokenStream> {
    let name = id(&prop.setter.name).expect(&prop.setter.name);
    let self_type = litstr(&format!("{}*", class.def.name)).expect(&prop.setter.name);

    let _type = if IMPLEMENTED_TYPES.contains(&prop._type.as_str()) {
        ty(format!("{}", &prop._type)).expect(&prop._type)
    } else {
        ty(&prop._type).expect(&prop._type)
    };
    let arg_type = litstr(&prop._type).expect(&prop._type);
    let code = quote! {
        pub fn #name(self, value: #_type) {
            let value = cpp!(unsafe [self as #self_type, value as #arg_type] -> #_type as #arg_type {
                self->#name(value);
                return value;
            });
        }
    };
    Ok(code)
}
