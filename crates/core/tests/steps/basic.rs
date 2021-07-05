use cucumber_rust::{t, Steps};

use crate::CoreWorld;

pub fn steps() -> Steps<CoreWorld> {
    let mut steps: Steps<CoreWorld> = Steps::new();

    steps.given_regex(r#"^module path "(.*)"$"#, |mut world, ctx| {
        let source =
            std::fs::read_to_string(std::fs::canonicalize(&ctx.matches[1]).unwrap()).unwrap();
        let key = world
            .ctx
            .with(|ctx| {
                let module = rquickjs::Module::new(ctx, "<module>", source)?;
                let evaled = module.eval()?;
                let value = rquickjs::IntoJs::into_js(evaled, ctx)?;
                let key = ctx.register(value);
                rquickjs::Result::Ok(key)
            })
            .unwrap();
        world.module = key;
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
