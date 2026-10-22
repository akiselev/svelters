//! OW-CSS measurement (ignored): parse and register every `*.css` in `$OWCSS_SHEETS` into a fresh
//! egui stylist, as the backend's `install_sheet` does, and report sheets, bytes and time.
//! Run in release: `OWCSS_SHEETS=<dir> cargo test --release -p svelters-style --test parse_cost -- --ignored --nocapture`.

use std::time::Instant;
use svelters_style::*;

#[test]
#[ignore = "measurement: needs OWCSS_SHEETS"]
fn parse_cost() {
    let dir = std::path::PathBuf::from(std::env::var("OWCSS_SHEETS").expect("OWCSS_SHEETS"));
    let mut sheets: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "css"))
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    sheets.sort();
    let run = |only: &dyn Fn(&str) -> bool| -> (usize, usize, f64, f64) {
        let set: Vec<&(String, String)> = sheets.iter().filter(|(n, _)| only(n)).collect();
        let bytes = set.iter().map(|(_, t)| t.len()).sum();
        let mut parse_us = Vec::new();
        let mut total_us = Vec::new();
        for _ in 0..200 {
            let mut stylist = Stylist::new(&tables::EGUI);
            let mut parse = 0.0;
            let t0 = Instant::now();
            for (_, text) in &set {
                let t = Instant::now();
                let sheet = Stylesheet::parse(text, &ParseOptions::default());
                parse += t.elapsed().as_secs_f64();
                let _ = stylist.add_sheet(&sheet, Origin::Author);
            }
            total_us.push(t0.elapsed().as_secs_f64() * 1e6);
            parse_us.push(parse * 1e6);
        }
        parse_us.sort_by(f64::total_cmp);
        total_us.sort_by(f64::total_cmp);
        (set.len(), bytes, parse_us[100], total_us[100])
    };
    for (label, f) in [
        ("startup (theme + components)", &(|_: &str| true) as &dyn Fn(&str) -> bool),
        ("theme only", &|n: &str| n.starts_with("theme-")),
        ("components (one HMR swap re-sends these)", &|n: &str| !n.starts_with("theme-")),
    ] {
        let (n, bytes, parse, total) = run(f);
        println!("{label}: {n} sheets, {bytes} bytes, parse {parse:.1} µs, parse + add_sheet {total:.1} µs (median of 200)");
    }
}
