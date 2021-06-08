use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use rayon::prelude::*;
use scraper::{Html, Selector};

use crate::{
    helpers::*,
    opts::Schema,
    types::*,
    visitors::{inheritance::get_inheritance_chain, methods::get_methods, props::get_props},
};

pub fn run_codegen(cmd: &Schema) -> Result<()> {
    let doc = open(
        qtwidgets("qtwidgets-module.html")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned(),
    )
    .unwrap();

    let iter: Vec<_> = doc
        .select(&Selector::parse(".tblName a").unwrap())
        .map(|node| {
            let file = node.value().attr("href").unwrap();
            let name = node.text().collect::<Vec<_>>().join("");
            let path = qtwidgets(file).unwrap().to_string_lossy().to_string();
            ClassDef { name, path }
        })
        .collect();
    let result: Vec<_> = iter
        .iter()
        // .par_iter()
        .map(move |class| -> Result<Class> {
            let doc = match open(class.path.clone()) {
                Ok(doc) => doc,
                Err(err) => panic!("ERROR {}: {}", err, class.path),
            };
            let mut inherits = vec![];
            get_inheritance_chain("docs/qt/qtwidgets/", &doc, &mut inherits)?;
            let class = class.to_owned().clone();
            println!("==== {}", class.name);
            let methods = get_methods(&doc).unwrap();
            let props = get_props(&doc).unwrap();
            Ok(Class {
                def: class,
                inherits,
                methods,
                props,
            })
        })
        .collect();

    let classes: HashMap<String, Class> = result
        .into_iter()
        .filter_map(|cls| cls.ok().map(|cls| (cls.def.name.clone(), cls)))
        .collect();

    println!("===== Finished =====");
    std::fs::write(
        &crate::opts::OPTS.schema,
        serde_json::to_string_pretty(&classes)?,
    )?;
    Ok(())
}
