use rquickjs::{bind, Ctx, Function, Result};
// use tokio::time::sleep;

#[bind(object)]
#[quickjs(rename = "setTimeout")]
pub fn set_timeout<'js>(_ctx: Ctx<'js>, _func: Function<'js>, _ms: u64) -> Result<()> {
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

pub fn init(ctx: Ctx) -> Result<()> {
    let global = ctx.globals();
    global.init_def::<SetTimeout>()?;
    Ok(())
}
