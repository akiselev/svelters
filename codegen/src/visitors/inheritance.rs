use anyhow::Result;
use scraper::{Html, Selector};

use crate::{helpers::*, types::*};

lazy_static! {
    static ref IGNORE_PARENT: Vec<&'static str> = vec!["QAccessibleObject", "QColorMap"];
}

pub fn get_inheritance_chain(prefix: &str, doc: &Html, result: &mut Vec<String>) -> Result<()> {
    let first_summary_sel = Selector::parse(".alignedsummary:nth-of-type(1) tr").unwrap();
    let rows = doc.select(&first_summary_sel);
    for row in rows {
        let res = (
            row.select(&Selector::parse("td:nth-of-type(1)").unwrap())
                .nth(0),
            row.select(&Selector::parse("td:nth-of-type(2) a").unwrap())
                .nth(0),
        );
        if let (Some(text), Some(anchor)) = res {
            if let (true, Some(file)) = (
                text.inner_html().contains("Inherits:"),
                anchor.value().attr("href"),
            ) {
                let name = anchor.inner_html();
                let path = match qtwidgets(file) {
                    Ok(path) => path,
                    Err(err) => panic!("ERROR {}: {}", err, file),
                }
                .to_string_lossy()
                .to_string();
                result.push(name.clone());
                let doc = match open(path.clone()) {
                    Ok(doc) => doc,
                    Err(err) => panic!("ERROR {}: {}", err, path),
                };
                if !IGNORE_PARENT.contains(&name.as_str()) {
                    get_inheritance_chain(prefix, &doc, result)?;
                }
            }
        }
    }

    Ok(())
}
