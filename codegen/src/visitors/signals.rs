use anyhow::Result;
use scraper::{ElementRef, Html, Selector};
use selectors::attr::CaseSensitivity;

use crate::{helpers::*, types::*};

pub fn get_signals(doc: &Html) -> Result<Vec<QFn>> {
    let first_summary_sel = Selector::parse(".fn").unwrap();
    let rows = doc.select(&first_summary_sel);
    let mut output = vec![];
    for row in rows {
        if let Some(code) = row.select(&Selector::parse("code").unwrap()).nth(0) {
            if code.inner_html().contains("[signal]") {
                if let Some(out) = get_qfn(row) {
                    output.push(out);
                }
            }
        }
    }

    Ok(output)
}

fn get_qfn(row: ElementRef) -> Option<QFn> {
    // println!("{}", res1);
    if let Some(name) = row.select(&Selector::parse(".name").unwrap()).nth(0) {
        let mut started = false;
        let next_siblings = name.next_siblings();
        let name = name.inner_html();
        let mut args = vec![];
        let mut is_const = false;
        let mut is_ptr = false;
        let mut is_ref = false;
        let mut has_default = false;
        let mut _type: Option<String> = None;
        let mut return_type: Option<String> = None;
        for node in next_siblings {
            let res = (node.value().is_text(), node.value().as_text());
            if started {
                let element = node.value().as_element();
                if let (Some(element), Some(true)) = (
                    ElementRef::wrap(node),
                    element.map(|res| res.has_class("type", CaseSensitivity::CaseSensitive)),
                ) {
                    if let Some(element) = element.select(&Selector::parse("a").unwrap()).nth(0) {
                        _type = Some(element.inner_html());
                    } else {
                        _type = Some(element.inner_html());
                    }
                }
            } else {
                let element = node.value().as_element();
                if let (Some(element), Some(true)) = (
                    ElementRef::wrap(node),
                    element.map(|res| res.has_class("type", CaseSensitivity::CaseSensitive)),
                ) {
                    if let Some(element) = element.select(&Selector::parse("a").unwrap()).nth(0) {
                        return_type = Some(element.inner_html());
                    } else {
                        return_type = Some(element.inner_html());
                    }
                }
            }
            if let (true, Some(text)) = res {
                let token = text.to_string();
                match token.as_str() {
                    "(" => started = true,
                    ")" => {
                        if let Some(_type) = _type.take() {
                            args.push(QArg {
                                is_const,
                                is_ptr,
                                is_ref,
                                has_default,
                                _type,
                            });
                        }
                        return Some(QFn {
                            name,
                            args,
                            return_type,
                        });
                    }
                    "," if started => {
                        if let Some(_type) = _type.take() {
                            args.push(QArg {
                                is_const,
                                is_ptr,
                                is_ref,
                                has_default,
                                _type,
                            });
                        }
                        is_const = false;
                        is_ptr = false;
                        is_ref = false;
                        has_default = false;
                    }
                    val if started && (val.starts_with(" =") || val.starts_with("=")) => {
                        has_default = true;
                        if let (true, Some(_type)) = (val.ends_with(")"), _type.take()) {
                            args.push(QArg {
                                is_const,
                                is_ptr,
                                is_ref,
                                has_default,
                                _type,
                            });
                        }
                        return Some(QFn {
                            name,
                            args,
                            return_type,
                        });
                    }
                    val => {
                        println!("=-= :{}", val)
                    }
                }
            }
        }
    }

    None
}
