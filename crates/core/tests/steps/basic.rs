use std::path::PathBuf;

use cucumber_rust::Steps;
use pretty_assertions::assert_eq;

use crate::CoreWorld;

pub fn steps() -> Steps<CoreWorld> {
    let mut steps: Steps<CoreWorld> = Steps::new();

    steps
        .given_regex(r#"^module at "(.*)"$"#, |mut world, ctx| {
            let mut path = PathBuf::new();
            path.push(env!("CARGO_MANIFEST_DIR"));
            path.push("tests/js");
            path.push(&ctx.matches[1]);
            let key = world
                .ctx
                .with(|ctx| {
                    let source = std::fs::read_to_string(
                        std::fs::canonicalize(path.clone())
                            .map_err(|err| anyhow::anyhow!("Path {:?} error: {}", path, err))?,
                    )
                    .unwrap();
                    let module = rquickjs::Module::new(ctx, "<module>", source)?;
                    let evaled = module.eval()?;
                    let value = rquickjs::IntoJs::into_js(evaled, ctx)?;
                    let key = ctx.register(value);
                    Result::<_, anyhow::Error>::Ok(key)
                })
                .unwrap();
            world.module = key;
            world
        })
        .when_regex(
            r#"^calling function "(.*)" with args \("(.*)", "(.*)", "(.*)", "(.*)", "(.*)"\)$"#,
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
                        let value = ctx
                            .deregister(world.module)
                            .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                        let module = value
                            .as_module()
                            .ok_or_else(|| anyhow::anyhow!("not a module"))?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.result = key;
                world
            },
        )
        .when_regex(
            r#"^calling function "(.*)" with args \("(.*)", "(.*)", "(.*)", "(.*)"\)$"#,
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
                        let value = ctx
                            .deregister(world.module)
                            .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                        let module = value
                            .as_module()
                            .ok_or_else(|| anyhow::anyhow!("not a module"))?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.result = key;
                world
            },
        )
        .when_regex(
            r#"^calling function "(.*)" with args \("(.*)", "(.*)", "(.*)"\)$"#,
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
                        let value = ctx
                            .deregister(world.module)
                            .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                        let module = value
                            .as_module()
                            .ok_or_else(|| anyhow::anyhow!("not a module"))?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.result = key;
                world
            },
        )
        .when_regex(
            r#"^calling function "(.*)" with args \("(.*), "(.*)""\)$"#,
            |mut world, ctx| {
                let func_name = &ctx.matches[1].to_owned();
                let args = (ctx.matches[2].to_owned(), ctx.matches[3].to_owned());
                let key = world
                    .ctx
                    .with(|ctx| {
                        let value = ctx
                            .deregister(world.module)
                            .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                        let module = value
                            .as_module()
                            .ok_or_else(|| anyhow::anyhow!("not a module"))?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call(args)?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.result = key;
                world
            },
        )
        .when_regex(
            r#"^calling function "(.*)" with args \("(.*)"\)$"#,
            |mut world, ctx| {
                let func_name = &ctx.matches[1].to_owned();
                let func_arg = &ctx.matches[2].to_owned();
                let key = world
                    .ctx
                    .with(|ctx| {
                        let value = ctx
                            .deregister(world.module)
                            .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                        let module = value
                            .as_module()
                            .ok_or_else(|| anyhow::anyhow!("not a module"))?;
                        let func: rquickjs::Function<'_> = module.get(func_name)?;
                        let value = func.call((func_arg,))?;
                        let key = ctx.register(value);
                        Result::<_, anyhow::Error>::Ok(key)
                    })
                    .unwrap();
                world.result = key;
                world
            },
        )
        .when_regex(r#"^calling function "(.*)"$"#, |mut world, ctx| {
            let func_name = &ctx.matches[1].to_string();
            let key = world
                .ctx
                .with(|ctx| {
                    let value = ctx
                        .deregister(world.module)
                        .ok_or_else(|| anyhow::anyhow!("key for module not found"))?;
                    let module = value
                        .as_module()
                        .ok_or_else(|| anyhow::anyhow!("no module found"))?;
                    let func: rquickjs::Function<'_> = module.get(func_name)?;
                    let value = func.call(())?;
                    let key = ctx.register(value);
                    Result::<_, anyhow::Error>::Ok(key)
                })
                .unwrap();
            world.module = key;
            world
        })
        .then_regex(r#"^return value is "(.*)"$"#, |world, ctx| {
            let expected = &ctx.matches[1].to_owned();
            world
                .ctx
                .with(|ctx| {
                    let value = ctx
                        .deregister(world.result)
                        .ok_or_else(|| anyhow::anyhow!("no return value not found"))?;
                    let result = value
                        .as_string()
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "expected string return value, found type {}",
                                value.type_name()
                            )
                        })?
                        .to_string()?;
                    assert_eq!(expected, &result);
                    Result::<_, anyhow::Error>::Ok(())
                })
                .unwrap();
            world
        })
        .then_regex(r#"^return value is \(\)$"#, |world, _| {
            world
                .ctx
                .with(|ctx| {
                    let value = ctx
                        .deregister(world.result)
                        .ok_or_else(|| anyhow::anyhow!("no return value not found"))?;
                    assert_eq!(
                        rquickjs::Value::new_undefined(ctx),
                        value,
                        "Return value should be undefined"
                    );
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
