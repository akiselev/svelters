use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use scraper::Html;
use syn::{self as s, parse_str as p};

use crate::types::{Class, ClassDef};

pub fn open(path: String) -> Result<Html> {
    Ok(Html::parse_document(
        std::fs::read_to_string(path)?.as_str(),
    ))
}

fn prefix<T: AsRef<str>>(prefix: &str, path: T) -> Result<PathBuf> {
    let qt = &crate::opts::OPTS.qt;
    if prefix.ends_with("/") {
        let path = format!("{}/{}{}", qt, prefix, path.as_ref());
        Ok(std::fs::canonicalize(path.split("#").nth(0).unwrap())?)
    } else {
        let path = format!("{}/{}/{}", qt, prefix, path.as_ref());
        Ok(std::fs::canonicalize(path.split("#").nth(0).unwrap())?)
    }
}

pub fn qtgui<T: AsRef<str>>(path: T) -> Result<PathBuf> {
    prefix("qtgui/", path)
}

pub fn qtwidgets<T: AsRef<str>>(path: T) -> Result<PathBuf> {
    let path = format!("{}", path.as_ref());
    let path = if path == "qabstractanimation.html" {
        "../qtcore/qabstractanimation.html".to_string()
    } else {
        path
    };
    let path = if path == "qabstracttransition.html" {
        "../qtcore/qabstractanimation.html".to_string()
    } else {
        path
    };
    let path = if path == "qaccessibleinterface.html" {
        "../qtgui/qaccessibleinterface.html".to_string()
    } else {
        path
    };
    let path = if path == "qaccessibleobject.html" {
        "../qtgui/qaccessibleobject.html".to_string()
    } else {
        path
    };
    let path = if path == "qobject.html" {
        "../qtcore/qobject.html".to_string()
    } else {
        path
    };
    prefix("qtwidgets/", path)
}

pub fn qtcore<T: AsRef<str>>(path: T) -> Result<PathBuf> {
    prefix("qtcore/", path)
}

pub fn get_namespace(path: &str) -> &'static str {
    match path {
        path if path.contains("qtcore/") => "QtCore",
        path if path.contains("qtgui/") => "QtGui",
        path if path.contains("qtwidgets/") => "QtWidgets",
        path if path.contains("qt3d/") => "Qt3D",
        _ => "???",
    }
}

pub fn read_schema() -> Result<HashMap<String, Class>> {
    let state = std::fs::read_to_string(&crate::opts::OPTS.schema)?;
    Ok(serde_json::from_str(&state)?)
}

pub(crate) fn id<T: AsRef<str>>(name: T) -> Result<s::Ident> {
    Ok(p::<s::Ident>(name.as_ref())?)
}

pub(crate) fn ty<T: AsRef<str>>(name: T) -> Result<s::Type> {
    Ok(p::<s::Type>(name.as_ref())?)
}

pub(crate) fn litstr<T: AsRef<str>>(name: T) -> Result<s::Lit> {
    Ok(p::<s::Lit>(&format!("\"{}\"", name.as_ref()))?)
}
