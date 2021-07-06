use anyhow::Result;
use svelters_core::prelude::{test_inline_has_member, test_inline_inherits_member};
use svelters_qt::{QApplication, QMainWindow, QObject, QT};

#[tokio::main]
async fn main() -> Result<()> {
    svelters::logging::init();
    tracing::info!("Starting...");
    testing_inline().unwrap();
    // Run the local task set.
    let mut rt = svelters::Runtime::new(svelters_qt::QT)?;
    rt.start("./app/index.ts").await?;
    Ok(())
}

#[no_mangle]
fn testing_inline() -> Result<()> {
    let res = test_inline_inherits_member::<QT, QObject>("className", -1)?;
    println!("QT::QObject inherits className: {}", res);
    let res = test_inline_has_member::<QT, QMainWindow>("show")?;
    println!("QT::QMainWindow has show: {}", res);
    let res = test_inline_inherits_member::<QT, QMainWindow>("show", -1)?;
    println!("QT::QMainWindow inherits show at -1: {}", res);
    let res = test_inline_inherits_member::<QT, QMainWindow>("show", 0)?;
    println!("QT::QMainWindow inherits show at 0: {}", res);
    let res = test_inline_inherits_member::<QT, QMainWindow>("show", 1)?;
    println!("QT::QMainWindow inherits show at 1: {}", res);
    let res = test_inline_inherits_member::<QT, QMainWindow>("show", 2)?;
    println!("QT::QMainWindow inherits show at 2: {}", res);
    let res = test_inline_inherits_member::<QT, QApplication>("className", -1)?;
    println!("QT::QApplication inherits className: {}", res);
    Ok(())
}
