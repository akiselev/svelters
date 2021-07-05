use std::time::Duration;

use rquickjs::{
    bind, BuiltinLoader, BuiltinResolver, Context, Ctx, FileResolver, Func, Function, IntoJs,
    Module, NativeLoader, Persistent, Rest, Result, Runtime, ScriptLoader, Value,
};
// use tokio::time::sleep;

#[bind(object)]
#[quickjs(rename = "setTimeout")]
pub fn set_timeout<'js>(ctx: Ctx<'js>, func: Function<'js>, ms: u64) -> Result<()> {
    // if ms > 0 {
    //     let func = Persistent::save(ctx, func);
    //     let context = Context::from_ctx(ctx)?;
    //     ctx.spawn(async move {
    //         sleep(Duration::from_millis(ms)).await;
    //         context
    //             .with(move |ctx| {
    //                 let func = func.restore(ctx).unwrap();
    //                 func.call::<_, ()>(())
    //             })
    //             .unwrap();
    //     });
    // } else {
    //     func.call(())?;
    // }
    Ok(())
}

pub fn init<'js>(ctx: Ctx<'js>) -> Result<()> {
    let global = ctx.globals();
    global.init_def::<SetTimeout>()?;
    Ok(())
}
