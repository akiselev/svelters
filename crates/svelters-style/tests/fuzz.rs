//! A deterministic fuzz smoke test: random and mutated sheets never panic the parser, the
//! cascade or the value parsers. (A coverage-guided `cargo fuzz` target needs nightly; this runs
//! on stable in CI.)

mod common;

use common::Tree;
use svelters_style::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const SEEDS: &[&str] = &[
    ".a > b:hover, :is(.c, #d) ~ e:has(+ f) { gap: calc(1px + 2em * 3); --x: var(--y, 4px); color: var(--x) !important }",
    "@media (min-width: 30em) and (prefers-color-scheme: dark) { @layer ui { a { padding: 1px 2px } } }",
    ".card { .t { width: min(10px, 2em) } &:focus-visible { flex-direction: row } }",
    "[value=\"3\" i] :nth-child(2n+1 of .x) :not(:checked):state(cad.pending) { flex-grow: clamp(0, 1, 2) }",
    "@layer a, b.c; @layer { x { gap: 0 } } @keyframes k { from { gap: 0 } }",
];

const TOKENS: &[&str] = &[
    "{", "}", "(", ")", "[", "]", ":", ";", ",", ".", "#", "&", ">", "+", "~", "*", "!important", "@media", "@layer", "var(", "calc(",
    "--", "/*", "*/", "\"", "'", "\\", "1px", "-", "%", "e", " ", "\n", ":has(", ":is(", ":not(", "\u{fffd}", "\0", "url(",
];

fn mutate(rng: &mut Rng, s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    for _ in 0..1 + rng.below(6) {
        let pos = rng.below(chars.len() + 1);
        match rng.below(3) {
            0 if !chars.is_empty() && pos < chars.len() => {
                chars.remove(pos);
            }
            1 => {
                let t = TOKENS[rng.below(TOKENS.len())];
                for (i, c) in t.chars().enumerate() {
                    chars.insert(pos + i, c);
                }
            }
            _ => {
                let end = (pos + rng.below(12)).min(chars.len());
                chars.drain(pos..end);
            }
        }
    }
    chars.into_iter().collect()
}

#[test]
fn never_panics() {
    let tree = Tree::parse("window#r.a\n  box.c.card :hovered\n    b#d.x[value=3] :checked\n    e.t\n    f");
    let states = [("cad.pending", StateMask(1 << 32))];
    let mut rng = Rng(0x5eed_cafe_f00d_1234);
    for round in 0..3000 {
        let text = if round % 7 == 0 {
            (0..rng.below(80)).map(|_| TOKENS[rng.below(TOKENS.len())]).collect::<String>()
        } else {
            let seed = SEEDS[rng.below(SEEDS.len())];
            mutate(&mut rng, seed)
        };
        let sheet = Stylesheet::parse(&text, &ParseOptions { states: Some(&states) });
        let mut stylist = Stylist::new(&tables::EGUI);
        stylist.add_sheet(&sheet, Origin::Author);
        let (inline, _) = DeclBlock::parse_inline(&text);
        let mut caches = MatchCaches::new();
        let mut parent = None;
        for el in tree.all() {
            parent = Some(stylist.compute(el, parent.as_ref(), Some(&inline), &ComputeContext::default(), &mut caches));
        }
        let _ = parse_selector_list(&text, Some(&states));
        let _ = MediaList::parse(&text);
    }
}
