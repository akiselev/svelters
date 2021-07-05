#![feature(min_type_alias_impl_trait)]
#![feature(generic_associated_types)]
#![allow(incomplete_features)]

use std::convert::Infallible;

use cucumber_rust::{async_trait, criteria::feature, futures::FutureExt, Context, Cucumber, World};

use rquickjs::RegisteryKey;
use svelters_core::prelude::*;

mod common;
use common::*;

mod steps;

pub struct CoreWorld {
    pub rt: rquickjs::Runtime,
    pub ctx: rquickjs::Context,
    /// Last loaded module
    pub module: RegisteryKey,
    /// Last Object created, which is target of tests
    pub subject: RegisteryKey,
    /// Result of last operation
    pub result: RegisteryKey,
}

#[async_trait(?Send)]
impl World for CoreWorld {
    type Error = rquickjs::Error;

    async fn new() -> Result<Self> {
        let rt = rquickjs::Runtime::new()?;
        let ctx = rquickjs::Context::full(&rt)?;
        let (resolver, loader) = Fw::get_loader();
        rt.set_loader(resolver, loader);
        let key = ctx
            .with(move |ctx| {
                svelters_core::console::init(ctx)?;
                svelters_core::timing::init(ctx)?;
                let key = ctx.register(rquickjs::Undefined.into_value(ctx));
                Result::Ok(key)
            })
            .unwrap();
        Ok(CoreWorld {
            rt,
            ctx,
            module: key.clone(),
            subject: key.clone(),
            result: key.clone(),
        })
    }
}

#[tokio::main]
async fn main() {
    Cucumber::<CoreWorld>::new()
        // Specifies where our feature files exist
        .features(&["./tests/features"])
        // Adds the implementation of our steps to the runner
        .steps(steps::basic::steps())
        // Add some global context for all the tests, like databases.
        // .context(Context::new().add(pool))
        // Add some lifecycle functions to manage our database nightmare
        // .before(feature("Core"), |ctx| {
        //     let pool = ctx.get::<SqlitePool>().unwrap().clone();
        //     async move { create_tables(&pool).await }.boxed()
        // })
        // .after(feature("Core"), |ctx| {
        //     let pool = ctx.get::<SqlitePool>().unwrap().clone();
        //     async move { drop_tables(&pool).await }.boxed()
        // })
        // Parses the command line arguments if passed
        .cli()
        // Runs the Cucumber tests and then exists
        .run_and_exit()
        .await
}

#[test]
fn basic_constructor() -> anyhow::Result<()> {
    let result = Runner::new(
        r#"
import { Object } from "Core";

export const testClassName = () => new Object().getClassName();
"#,
    )
    .run("basic_constructor", |ctx, module| {
        let func: rquickjs::Function<'_> = module.get("name")?;
        let res: String = func.call(())?;
        Ok(res)
    })?;
    assert_eq!(result, "Obje1ct");
    Ok(())
}
