//! AU1b: `#[svelters_automation::test]`. Each body runs in process (kittest, the default) or out of
//! process (`SVELTERS_E2E=process`, against `svelters-run`) unchanged. The gallery tests are
//! `crates/svelters-egui/tests/gallery.rs`'s first two, rewritten with locators. The gallery uses
//! Vite's `import.meta.glob`, so they run its built bundle (`pnpm build`), as gallery.rs does.

use svelters_automation::e2e::prelude::*;

/// A 30 s timer on the virtual clock costs frames, not wall time; `performance.now()` agrees.
#[svelters_automation::test(app = "../../fixtures/counter-egui/src/Clock.svelte", seed = 7)]
fn a_30_s_timer_fires_on_virtual_time(d: &mut E2e) -> TestResult {
    let wall = std::time::Instant::now();
    expect(d.get_by_text_exact("Fired: no")).to_be_visible()?;
    d.get_by_role("button", name("Start")).click()?;
    let t0 = d.clock().now_ms()?.expect("a virtual clock");
    d.clock().run_for(29_000.0)?;
    expect(d.get_by_text_exact("Fired: no")).to_be_visible()?;
    expect(d.get_by_text("Fired: after 30")).timeout(5_000.0).to_be_visible()?;
    let t1 = d.clock().now_ms()?.unwrap();
    assert!(t1 - t0 >= 30_000.0, "{t0} → {t1}");
    assert!(wall.elapsed().as_secs() < 30, "virtual time took {:?} of wall time", wall.elapsed());
    Ok(())
}

fn rolls(d: &E2e) -> Result<String, Error> {
    Ok(d.get_by_text("Rolls:").text_content()?.unwrap_or_default())
}

/// The seed makes `Math.random` replayable: the same seed rolls the same, another seed differs.
#[svelters_automation::test(app = "../../fixtures/counter-egui/src/Clock.svelte", seed = 11, iterations = 2)]
fn the_seed_replays_math_random(d: &mut E2e) -> TestResult {
    let mine = rolls(d)?;
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui/src/Clock.svelte");
    let clock = svelters_automation::clock::ClockMode::Virtual;
    let again = E2e::in_process(&app, clock, d.seed(), 16.0)?;
    assert_eq!(rolls(&again)?, mine, "seed {}", d.seed());
    let other = E2e::in_process(&app, clock, d.seed() + 100, 16.0)?;
    assert_ne!(rolls(&other)?, mine, "seed {} vs {}", d.seed(), d.seed() + 100);
    Ok(())
}

fn open<'a>(d: &'a E2e, page: &str) -> Result<(), Error> {
    d.get_by_role("button", name(page)).click()?;
    expect(d.get_by_text_exact(&format!("Page: {page}"))).to_be_visible()
}

/// gallery.rs's `inputs_page_binds_every_value`, with locators.
#[svelters_automation::test(app = "../../fixtures/gallery-egui/dist/app.js")]
fn gallery_inputs_page_binds_every_value(d: &mut E2e) -> TestResult {
    open(d, "inputs")?;
    expect(d.get_by_text_exact("group: a")).to_be_visible()?;
    d.get_by_role("radio", name("Gamma")).click()?;
    expect(d.get_by_text_exact("group: c")).to_be_visible()?;
    expect(d.get_by_text_exact("checked: true false")).to_be_visible()?;
    d.get_by_role("radio", name("Second")).click()?;
    expect(d.get_by_text_exact("checked: false true")).to_be_visible()?;
    d.get_by_role("switch", name("Wi-Fi")).click()?;
    expect(d.get_by_text_exact("active: true")).to_be_visible()?;
    // Not here: gallery.rs's `bind:high` step. The range slider is one scene node ("Range") with two
    // AccessKit handles; `input.set_value` has no handle target yet (AU1b milestone, follow-ups).
    d.get_by_role("spinbutton", name("Width")).set_value(json!(25))?;
    expect(d.get_by_text_exact("width: 10")).to_be_visible()?;
    d.get_by_role("link", name("Svelte")).click()?;
    expect(d.get_by_text_exact("link clicks: 1")).to_be_visible()?;
    Ok(())
}

/// gallery.rs's `containers_page_lays_out_and_switches`, with locators.
#[svelters_automation::test(app = "../../fixtures/gallery-egui/dist/app.js")]
fn gallery_containers_page_switches(d: &mut E2e) -> TestResult {
    open(d, "containers")?;
    for l in ["Panel 140", "Body", "Left side", "Inside the frame", "Stack page one"] {
        expect(d.get_by_text_exact(l)).to_be_visible()?;
    }
    // Not here: "the other stack page is hidden". `visible` reads a stack's inactive child as visible
    // (AU1b milestone, follow-ups), so only the switch's positive side is checked.
    d.get_by_role("button", name("Show two")).click()?;
    expect(d.get_by_text_exact("Stack page two")).to_be_visible()?;
    let (panel, body) = (d.get_by_text_exact("Panel 140").info()?, d.get_by_text_exact("Body").info()?);
    let left = |v: &serde_json::Value| v["rect"][0].as_f64();
    if let (Some(p), Some(b)) = (left(&panel), left(&body)) {
        assert!(b - p >= 139.0, "{panel} {body}");
    }
    Ok(())
}
