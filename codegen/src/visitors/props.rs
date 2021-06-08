use anyhow::Result;
use scraper::{ElementRef, Html, Selector};

use crate::{helpers::*, types::*};

pub fn get_props(doc: &Html) -> Result<Vec<Property>> {
    let first_summary_sel = Selector::parse(".prop table.alignedsummary").unwrap();
    let rows = doc.select(&first_summary_sel);
    let mut output = vec![];
    for (_index, row) in rows.enumerate() {
        // println!("{}", _index);
        if let Some(out) = get_prop(row) {
            output.push(out);
        }
    }

    Ok(output)
}

fn get_prop(table: ElementRef) -> Option<Property> {
    let selector = Selector::parse("tr").unwrap();
    let mut rows = table.select(&selector);
    // println!("{:?}", table);
    let (getter, setter) = (rows.next(), rows.next());
    let (getter, setter) = if let (Some(getter), Some(setter)) = (getter, setter) {
        (getter, setter)
    } else {
        return None;
    };

    let selector = Selector::parse("td").unwrap();
    let mut cols = getter.select(&selector);
    let (lgetter, rgetter) = (cols.next().unwrap(), cols.next().unwrap());
    let mut cols = setter.select(&selector);
    let (_, rsetter) = (cols.next().unwrap(), cols.next().unwrap());

    let _type = lgetter.inner_html().replace(" ", "");
    let selector = Selector::parse(".name b").unwrap();
    let mut name = rgetter.select(&selector);
    let name = name.next().unwrap().inner_html();
    let text = rgetter
        .text()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join("");
    let getter = Accessor {
        name,
        is_const: text.contains("const"),
        is_ref: text.contains("&"),
        is_ptr: text.contains("*"),
    };

    let mut name = rsetter.select(&selector);
    let name = name.next().unwrap().inner_html();
    let text = rsetter
        .text()
        .map(|t| t.to_string())
        .collect::<Vec<_>>()
        .join("");
    let setter = Accessor {
        name,
        is_const: text.contains("const"),
        is_ref: text.contains("&"),
        is_ptr: text.contains("*"),
    };

    Some(Property {
        getter,
        setter,
        _type,
    })
}
