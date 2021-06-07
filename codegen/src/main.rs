#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use dashmap::DashMap;
use parking_lot::FairMutex;
use rayon::prelude::*;
use scraper::{Html, Selector};

pub mod helpers;
pub mod types;
pub mod visitors;
use helpers::*;
use types::*;
use visitors::*;

fn main() {
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
            println!("{} => {}", name, path);
            ClassDef { name, path }
        })
        .collect();
    let result: Vec<_> = iter
        .par_iter()
        .take(20)
        .map(move |class| -> Result<QClass> {
            let doc = match open(class.path.clone()) {
                Ok(doc) => doc,
                Err(err) => panic!("ERROR {}: {}", err, class.path),
            };
            let mut inherits = vec![];
            get_inheritance_chain("docs/qt/qtwidgets/", &doc, &mut inherits)?;
            let signals = get_signals(&doc).unwrap();
            let class = class.to_owned().clone();
            Ok(QClass {
                class,
                inherits,
                signals,
            })
        })
        .collect();

    let classes: HashMap<String, QClass> = result
        .into_iter()
        .filter_map(|cls| cls.ok().map(|cls| (cls.class.name.clone(), cls)))
        .collect();

    println!("===== Finished =====");
    std::fs::write(
        "./output.txt",
        serde_json::to_string_pretty(&classes).unwrap(),
    )
    .unwrap();
}
