//! CX1 timing: `cargo run --release -p svelters-compiler --example check_timing -- [--backend gtk] <file.svelte>...`
//!
//! Per file, the median over `N` runs (warm, in process) of:
//! - `compile`: the full compile (code and source map);
//! - `check`: a fresh check (no previous state);
//! - `keystroke`: re-checking an edited buffer with its `FileState`, one character typed per check
//!   (inside the last text attribute value, so the buffer stays valid);
//! - `unchanged`: re-checking the same buffer (the state answers);
//! - `syntax error`: a buffer that does not parse, with the last complete diagnostics carried over.

use std::time::Instant;
use svelters_compiler::check::{FileState, check};
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;

const N: usize = 50;

fn median(mut v: Vec<u128>) -> f64 {
    v.sort();
    v[v.len() / 2] as f64 / 1000.0
}

fn time(mut f: impl FnMut(usize)) -> f64 {
    for i in 0..3 {
        f(i);
    }
    median(
        (0..N)
            .map(|i| {
                let t = Instant::now();
                f(i);
                t.elapsed().as_micros()
            })
            .collect(),
    )
}

fn main() {
    let mut backend = "gtk".to_string();
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--backend" {
            backend = args.next().expect("--backend needs a value");
        } else {
            files.push(a);
        }
    }
    let b = SpecBackend::builtin(&backend).unwrap();
    println!("| file | bytes | compile | check | keystroke | unchanged | syntax error |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        let name = f.rsplit('/').next().unwrap().to_string();
        let opts = CompileOptions::new(name.clone(), &b);
        let full = time(|_| {
            std::hint::black_box(compile(&src, &opts));
        });
        let fresh = time(|_| {
            std::hint::black_box(check(&name, &src, &b, &mut FileState::new()));
        });
        // typing into the last `label="..."`-like value: insert one more character each time
        let at = src.rfind("=\"").map(|i| i + 2).unwrap_or(src.len());
        let mut st = FileState::new();
        let mut buf = src.clone();
        check(&name, &buf, &b, &mut st);
        let keystroke = time(|i| {
            buf.insert(at, (b'a' + (i % 26) as u8) as char);
            std::hint::black_box(check(&name, &buf, &b, &mut st));
        });
        let unchanged = time(|_| {
            std::hint::black_box(check(&name, &buf, &b, &mut st));
        });
        let mut st = FileState::new();
        check(&name, &src, &b, &mut st);
        let broken = format!("{}{{1 + }}{}", &src[..at - 1], &src[at - 1..]);
        let syntax = time(|i| {
            let s = format!("{broken}{}", " ".repeat(i % 2));
            std::hint::black_box(check(&name, &s, &b, &mut st));
        });
        println!("| {f} | {} | {full:.2} ms | {fresh:.2} ms | {keystroke:.2} ms | {unchanged:.3} ms | {syntax:.2} ms |", src.len());
    }
}
