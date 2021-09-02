use anyhow::Result;
use svelters_qt::{QApplication, QMainWindow, QObject, QT};

#[tokio::main]
async fn main() -> Result<()> {
    svelters::logging::init();
    tracing::info!("Starting...");
    // Run the local task set.
    let mut rt = svelters::Runtime::new(svelters_qt::QT)?;
    rt.start("./app/index.ts").await?;
    Ok(())
}
