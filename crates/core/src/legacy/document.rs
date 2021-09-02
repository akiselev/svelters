use rquickjs::{bind, Ctx, Rest, Result as JsResult, Value};

#[bind(object)]
pub mod console {
    use super::*;

    pub fn log<'js>(args: Rest<Value<'js>>) {
        let args: Vec<Value<'js>> = args.into_inner();
        tracing::info!(?args, "console.log");
    }

    pub fn error<'js>(args: Rest<Value<'js>>) {
        let args: Vec<Value<'js>> = args.into_inner();
        tracing::error!(?args, "console.error");
    }

    pub fn warn<'js>(args: Rest<Value<'js>>) {
        let args: Vec<Value<'js>> = args.into_inner();
        tracing::warn!(?args, "console.warn");
    }

    pub fn debug<'js>(args: Rest<Value<'js>>) {
        let args: Vec<Value<'js>> = args.into_inner();
        tracing::debug!(?args, "console.debug");
    }
}

pub fn init<'js>(ctx: Ctx<'js>) -> JsResult<()> {
    tracing::info!("Initializing console");
    let glob = ctx.globals();
    glob.init_def::<Console>()?;
    Ok(())
}
