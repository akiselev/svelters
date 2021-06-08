use std::collections::{HashMap, HashSet};

use anyhow::Result;
use quote::quote;

use crate::{
    code::{props::codegen_props, traits::*},
    constants::*,
    helpers::ty,
    types::*,
};

mod props;
mod traits;

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
        let props = codegen_props(cls)?;
        let constructor =
            if !cls.is_abstract() && !CODEGEN_IGNORE_CONSTRUCTOR.contains(&cls.def.name.as_str()) {
                codegen_constructor(cls.def.name.as_str())?
            } else {
                quote! {}
            };
        let types = codegen_types(cls)?;
        let _type = ty(cls.def.name.as_str())?;
        code.push(quote! {
            #types
            #obj_impl
            #interface

            #constructor
            #props
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
        "use cast_trait_object::{{dyn_cast, dyn_upcast, DynCastExt}};\nuse cpp::{{cpp, cpp_class}};\nuse qttypes::*;\nuse crate::{{types::*,QObject, QWidget,QObjectPtr, QWidgetPtr, QRef}};\n\n{}\n",
        codegen_headers(
            classes
                .values()
                .filter(|c| !c.def.name.contains("<"))
                .filter(|c| !c.def.name.contains("::"))
        )?
    ) + output.to_string().as_str())
}
