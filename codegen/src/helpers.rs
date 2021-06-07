use std::path::PathBuf;

use anyhow::Result;
use scraper::Html;

pub fn open(path: String) -> Result<Html> {
    Ok(Html::parse_document(
        std::fs::read_to_string(path)?.as_str(),
    ))
}

fn prefix<T: AsRef<str>>(prefix: &str, path: T) -> Result<PathBuf> {
    if prefix.ends_with("/") {
        let path = format!("{}{}", prefix, path.as_ref());
        Ok(std::fs::canonicalize(path.split("#").nth(0).unwrap())?)
    } else {
        let path = format!("{}/{}", prefix, path.as_ref());
        Ok(std::fs::canonicalize(path.split("#").nth(0).unwrap())?)
    }
}

pub fn qtgui<T: AsRef<str>>(path: T) -> Result<PathBuf> {
    prefix("docs/qt/qtgui/", path)
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
    prefix("docs/qt/qtwidgets/", path)
}

pub fn qtcore<T: AsRef<str>>(path: T) -> Result<PathBuf> {
    prefix("docs/qt/qtcore/", path)
}
