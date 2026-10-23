//! WG3s: records one real trackpad scroll gesture with its OS momentum, as a replay fixture for
//! pin 10 (`plans/milestones/WG3s-scroll-references.md` §5). macOS only; needs Input Monitoring for
//! the terminal. Usage: `scroll_record <out.json> [wait_s]`, then one two-finger flick on the
//! trackpad within `wait_s` seconds (default 60). It stops when the flick's momentum ends.

fn main() {
    #[cfg(target_os = "macos")]
    {
        use svelters_conformance::macos::scroll_events::{record, to_json};
        let out = std::env::args()
            .nth(1)
            .expect("usage: scroll_record <out.json>");
        let natural = std::process::Command::new("defaults")
            .args(["read", "-g", "com.apple.swipescrolldirection"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let wait: f64 = std::env::args()
            .nth(2)
            .and_then(|w| w.parse().ok())
            .unwrap_or(60.0);
        eprintln!("recording: flick two fingers on the trackpad once ({wait} s to start)");
        let events = record(wait).unwrap_or_else(|e| panic!("{e}"));
        let gesture = events.iter().filter(|e| e.phase != 0).count();
        let momentum = events.iter().filter(|e| e.momentum != 0).count();
        let note = format!(
            "macOS {} trackpad flick; natural scrolling (com.apple.swipescrolldirection) = {natural}; {gesture} gesture and {momentum} momentum events",
            svelters_conformance::macos::macos_version()
        );
        std::fs::write(&out, to_json(&events, &note)).expect("write the recording");
        eprintln!(
            "wrote {out}: {} events ({gesture} gesture, {momentum} momentum)",
            events.len()
        );
    }
    #[cfg(not(target_os = "macos"))]
    eprintln!("scroll_record: macOS only");
}
