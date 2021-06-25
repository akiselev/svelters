use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    svelters::logging::init();
    tracing::info!("Starting...");

    // Run the local task set.
    let mut rt = svelters::Runtime::new(svelters_qt::QT)?;
    rt.load_module::<"QtCore">()?;
    rt.start("./app/index.ts").await?;
    Ok(())
}
