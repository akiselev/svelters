#![allow(clippy::module_inception)]
use rquickjs::{bind, Ctx, Rest, Result as JsResult};

#[bind(object)]
pub mod console {
    use rquickjs::Coerced;

    use super::*;

    pub fn log(_ctx: Ctx, args: Rest<Coerced<String>>) -> JsResult<()> {
        let args: Vec<Coerced<String>> = args.into_inner();
        let line = args
            .into_iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>()
            .join(" ");
        tracing::info!("{}", line);
        Ok(())
    }

    pub fn error(_ctx: Ctx, args: Rest<Coerced<String>>) -> JsResult<()> {
        let args: Vec<Coerced<String>> = args.into_inner();
        let line = args
            .into_iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>()
            .join(" ");
        tracing::error!("{}", line);
        Ok(())
    }

    pub fn warn(_ctx: Ctx, args: Rest<Coerced<String>>) -> JsResult<()> {
        let args: Vec<Coerced<String>> = args.into_inner();
        let line = args
            .into_iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>()
            .join(" ");
        tracing::warn!("{}", line);
        Ok(())
    }

    pub fn debug(_ctx: Ctx, args: Rest<Coerced<String>>) -> JsResult<()> {
        let args: Vec<Coerced<String>> = args.into_inner();
        let line = args
            .into_iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>()
            .join(" ");
        tracing::debug!("{}", line);
        Ok(())
    }
}

pub fn init(ctx: Ctx) -> JsResult<()> {
    tracing::info!("Initializing console");
    let glob = ctx.globals();
    glob.init_def::<Console>()?;
    Ok(())
}
