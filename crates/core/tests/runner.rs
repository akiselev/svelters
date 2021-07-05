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

pub use common::CoreWorld;

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
