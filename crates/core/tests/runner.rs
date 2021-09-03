#![feature(min_type_alias_impl_trait)]
#![feature(generic_associated_types)]
#![feature(specialization)]
#![allow(incomplete_features)]

use cucumber_rust::{criteria::feature, futures::FutureExt, Cucumber};

mod common;

mod steps;

pub use common::CoreWorld;
use svelters_core::prelude::cleanup_svelters;

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
        .after(feature("Core"), |ctx| {
            async move {
                println!(" ");
                println!(" ");
            }
            .boxed()
        })
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
