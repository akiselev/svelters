// Build with RUSTFLAGS="--cfg feature_inline_bad" to see an inline-template error; PROBE_BAD=1 for build.rs errors.
static APP_QBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/App.qbc"));
static STATS: &str = include_str!(concat!(env!("OUT_DIR"), "/stats.txt"));
const VIA_MACRO: &str = svelte_macro::include_svelte!("ui/Macro.svelte");
#[cfg(feature_inline_bad)]
const INLINE_BAD: &str = svelte_macro::svelte!(r#"<window><buton>x</buton></window>"#);
const INLINE: &str = svelte_macro::svelte!(r#"<window title="t"><label>hi</label></window>"#);

fn main() {
    println!("build.rs: {STATS}");
    println!("embedded bytecode: {} B; include_svelte! js: {} B; svelte! js: {} B", APP_QBC.len(), VIA_MACRO.len(), INLINE.len());
}
