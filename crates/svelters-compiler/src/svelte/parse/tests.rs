use super::*;

fn json(src: &str) -> String {
    serde_json::to_string(&parse(src, &ParseOptions::default()).unwrap()).unwrap()
}

#[test]
fn probe() {
    println!("{}", json("<div a=\"x\" {b} class:c></div><svelte:options runes />"));
}
