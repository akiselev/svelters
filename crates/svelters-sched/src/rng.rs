//! A small seeded PRNG (xoshiro256** seeded through SplitMix64): the same seed gives the same
//! sequence on every platform, without a dependency.

use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut x = seed;
        Rng { s: [splitmix(&mut x), splitmix(&mut x), splitmix(&mut x), splitmix(&mut x)] }
    }

    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }

    /// Uniform in `0..n` (`n > 0`), without modulo bias.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Rng::below(0)");
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }

    /// Uniform in `r` (non-empty).
    pub fn range(&mut self, r: Range<u64>) -> u64 {
        assert!(r.start < r.end, "Rng::range on an empty range");
        r.start + self.below(r.end - r.start)
    }

    /// True with probability `p` (clamped to 0..=1).
    pub fn bool(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p.clamp(0.0, 1.0)
    }
}
