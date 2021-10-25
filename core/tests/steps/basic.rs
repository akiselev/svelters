use std::path::PathBuf;

use cucumber_rust::Steps;
use pretty_assertions::assert_eq;
use svelters_core::prelude::{initialize_gui_context, Document};

use crate::CoreWorld;

pub fn steps() -> Steps<CoreWorld> {
    let mut steps: Steps<CoreWorld> = Steps::new();

    steps
        .given_regex(r#"^module at "(.+)"$"#, |mut world, ctx| {
            let mut path = PathBuf::new();
            let name = (*&ctx.matches[1]).clone();
            path.push(env!("CARGO_MANIFEST_DIR"));
            path.push("tests/js");
            path.push(&name);
            let key = world
                .ctx
                .with(|ctx| {
                    initialize_gui_context(ctx)?;
                    let source = std::fs::read_to_string(
                        std::fs::canonicalize(path.clone())
                            .map_err(|err| anyhow::anyhow!("Path {:?} error: {}", path, err))?,
                    )
                    .unwrap();
                    let module = rquickjs::Module::new(ctx, name, source)?;
                    let evaled = module.eval()?;
                    let value = rquickjs::IntoJs::into_js(evaled, ctx)?;
                    let key = ctx.register(value);
                    Result::<_, anyhow::Error>::Ok(key)
                })
                .unwrap();
            world.set_module(key).unwrap();
            world
        })
        .when_regex(
            r#"^calling function "([^x"]*)" with args \("([^"]*)"\)$"#,
            |mut world, ctx| {
                let func_name = &ctx.matches[1].to_owned();
                let func_arg = &ctx.matches[2].to_owned();
                let key = world
                    .ctx
                    .with(|ctx| {
                        let module = world.get_module(ctx)?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call::<_, rquickjs::Value<'_>>((func_arg,))?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.set_result(key).unwrap();
                world
            },
        )
        .when_regex(
            r#"^calling function "([^x"]*)" with args \("([^"]*)", "([^"]*)"\)$"#,
            |mut world, ctx| {
                let func_name = &ctx.matches[1].to_owned();
                let args = (ctx.matches[2].to_owned(), ctx.matches[3].to_owned());
                let key = world
                    .ctx
                    .with(|ctx| {
                        let module = world.get_module(ctx)?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call::<_, rquickjs::Value<'_>>(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.set_result(key).unwrap();
                world
            },
        )
        .when_regex(
            r#"^calling function "([^x"]*)" with args \("([^"]*)", "([^"]*)", "([^"]*)"\)$"#,
            |mut world, ctx| {
                let func_name = &ctx.matches[1].to_owned();
                let args = (
                    ctx.matches[2].to_owned(),
                    ctx.matches[3].to_owned(),
                    ctx.matches[4].to_owned(),
                );
                let key = world
                    .ctx
                    .with(|ctx| {
                        let module = world.get_module(ctx)?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call::<_, rquickjs::Value<'_>>(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.set_result(key).unwrap();
                world
            },
        )
        .when_regex(
            r#"^calling function "([^x"]*)" with args \("([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)"\)$"#,
            |mut world, ctx| {
                let func_name = ctx.matches[1].to_owned();
                let args = (
                    ctx.matches[2].to_owned(),
                    ctx.matches[3].to_owned(),
                    ctx.matches[4].to_owned(),
                    ctx.matches[5].to_owned(),
                );
                let key = world
                    .ctx
                    .with(|ctx| {
                        let module = world.get_module(ctx)?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call::<_, rquickjs::Value<'_>>(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.set_result(key).unwrap();
                world
            },
        )
        .when_regex(
            r#"^calling function "([^x"]*)" with args \("([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)", "([^"]*)"\)$"#,
            |mut world, ctx| {
                let func_name = ctx.matches[1].to_owned();
                let args = (
                    ctx.matches[2].to_owned(),
                    ctx.matches[3].to_owned(),
                    ctx.matches[4].to_owned(),
                    ctx.matches[5].to_owned(),
                    ctx.matches[6].to_owned(),
                );
                let key = world
                    .ctx
                    .with(|ctx| {
                        let module = world.get_module(ctx)?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call::<_, rquickjs::Value<'_>>(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.set_result(key).unwrap();
                world
            },
        )
        .when_regex(r#"^calling function "([^"]*)"$"#, |mut world, ctx| {
            let func_name = &ctx.matches[1].to_string();
            let key = world
                .ctx
                .with(|ctx| {
                    let module = world.get_module(ctx)?;
                    let func: rquickjs::Function<'_> = module.get(func_name)?;
                    let value = func.call::<_, rquickjs::Value<'_>>(())?;
                    let key = ctx.register(value);
                    Result::<_, anyhow::Error>::Ok(key)
                })
                .unwrap();
            world.set_result(key).unwrap();
            world
        })
        .then_regex(r#"^return value is "([^"]*)"$"#, |world, ctx| {
            let expected = &ctx.matches[1].to_owned();
            world
                .ctx
                .with(|ctx| {
                    let value = world.get_result(ctx)?;
                    let result = value
                        .as_string()
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "expected string return value, found type {}",
                                value.type_name()
                            )
                        })?
                        .to_string()?;
                    assert_eq!(&result, expected);
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        })
        .then_regex(r#"^return value is true$"#, |world, _| {
            world
                .ctx
                .with(|ctx| {
                    let value = world.get_result(ctx)?;
                    assert_eq!(
                        value,
                        rquickjs::Value::new_bool(ctx, true),
                        "Return value should be true"
                    );
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        })
        .then_regex(r#"^return value is false$"#, |world, _| {
            world
                .ctx
                .with(|ctx| {
                    let value = world.get_result(ctx)?;
                    assert_eq!(
                        rquickjs::Value::new_bool(ctx, false),
                        value,
                        "Return value should be true"
                    );
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        })
        .then_regex(r#"^return value is \(\)$"#, |world, _| {
            world
                .ctx
                .with(|ctx| {
                    let value = world.get_result(ctx)?;
                    assert_eq!(
                        value,
                        rquickjs::Value::new_undefined(ctx),
                        "Return value should be undefined"
                    );
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        })
        .then_regex(r#"^return value is object$"#, |world, _| {
            world
                .ctx
                .with(|ctx| {
                    let value = world.get_result(ctx)?;
                    assert_eq!(value.is_object(), true, "Return value should be an object");
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        });

    // builder
    //     .given_regex_async(
    //         r#"^I have an encryptor initialized with input "([\w\s!]+)"$"#,
    //         t!(|mut world, texts_to_encrypt, _step| {
    //             world.encryptor.input = Cow::Owned(texts_to_encrypt[1].to_owned());
    //             world
    //         }),
    //     )
    //     .then_regex_async(
    //         r#"^I should see "([\w\s!]+)" in the encryptor's input field$"#,
    //         t!(|world, expected_texts, _step| {
    //             assert_eq!(expected_texts[1], world.encryptor.input);
    //             world
    //         }),
    //     )
    //     .when_async(
    //         "I encrypt the encryptor's input",
    //         t!(|world, _step| {
    //             world.encryptor.write_encrypted();
    //             world
    //         }),
    //     )
    //     .then_async(
    //         "testfile.txt exists",
    //         t!(|_world, _step| {
    //             let testfile_path = Path::new("./testfile.txt");
    //             assert_eq!(testfile_path.exists(), true);
    //             _world
    //         }),
    //     )
    //     .then_async(
    //         "testfile.txt is not empty",
    //         t!(|mut world, _step| {
    //             let enc_message = fs::read("./testfile.txt").expect("Could not read test file.");
    //             world.encrypted_base64 = base64::encode(&enc_message);

    //             assert_eq!(world.encrypted_base64.len() > (0 as usize), true);
    //             world
    //         }),
    //     )
    //     .when_async(
    //         "I decrypt testfile.txt",
    //         t!(|mut world, _step| {
    //             world.decrypt_result = world.decryptor.read_decrypted();
    //             world
    //         }),
    //     )
    //     .then_regex_async(
    //         r#"^the decrypted result should be "([\w\s!]+)"$"#,
    //         t!(|mut world, expected_texts, _step| {
    //             assert_eq!(expected_texts[1], world.decrypt_result);
    //             world
    //         }),
    //     );

    steps
}
