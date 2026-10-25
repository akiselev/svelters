//! Damage → scissor regions (architecture §8, brief "Damage"): the renderer redraws only the damaged
//! device rects, each under its own scissor, or their union when one pass over it is cheaper.

use skelter_base::geom::{DeviceRect, DeviceSize};

/// Above this many disjoint rects, one union pass is used.
pub const MAX_REGIONS: usize = 8;
/// Use the union when it covers at most this much more area than the rects themselves (the cost of
/// re-encoding every batch per region outweighs shading a few extra pixels).
pub const UNION_SLACK: f32 = 1.5;

fn area(r: &DeviceRect) -> u64 {
    r.w as u64 * r.h as u64
}

fn overlaps(a: &DeviceRect, b: &DeviceRect) -> bool {
    a.x < b.x + b.w as i32 && b.x < a.x + a.w as i32 && a.y < b.y + b.h as i32 && b.y < a.y + a.h as i32
}

/// The scissor regions for one frame: the whole target when `damage` is empty, else the damage
/// clipped to the target with overlapping rects merged. Empty when all damage is off-target.
pub fn plan(damage: &[DeviceRect], target: DeviceSize) -> Vec<DeviceRect> {
    let full = DeviceRect { x: 0, y: 0, w: target.w, h: target.h };
    if full.is_empty() {
        return Vec::new();
    }
    if damage.is_empty() {
        return vec![full];
    }
    let mut rects: Vec<DeviceRect> = damage.iter().map(|r| r.clip_to(target)).filter(|r| !r.is_empty()).collect();
    // Merge overlapping rects until none overlap.
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if overlaps(&rects[i], &rects[j]) {
                    let u = rects[i].union(&rects[j]);
                    rects[i] = u;
                    rects.swap_remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    if rects.len() <= 1 {
        return rects;
    }
    let union = rects.iter().skip(1).fold(rects[0], |u, r| u.union(r));
    let sum: u64 = rects.iter().map(area).sum();
    if rects.len() > MAX_REGIONS || area(&union) as f32 <= sum as f32 * UNION_SLACK {
        vec![union]
    } else {
        rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> DeviceRect {
        DeviceRect { x, y, w, h }
    }
    const T: DeviceSize = DeviceSize { w: 100, h: 100 };

    #[test]
    fn empty_damage_is_the_whole_target() {
        assert_eq!(plan(&[], T), vec![r(0, 0, 100, 100)]);
    }

    #[test]
    fn far_apart_rects_stay_separate_and_near_ones_merge() {
        assert_eq!(plan(&[r(0, 0, 10, 10), r(90, 90, 10, 10)], T), vec![r(0, 0, 10, 10), r(90, 90, 10, 10)]);
        assert_eq!(plan(&[r(0, 0, 10, 10), r(10, 0, 10, 10)], T), vec![r(0, 0, 20, 10)]);
        assert_eq!(plan(&[r(0, 0, 10, 10), r(5, 5, 10, 10)], T), vec![r(0, 0, 15, 15)]);
    }

    #[test]
    fn damage_is_clipped_to_the_target() {
        assert_eq!(plan(&[r(-5, -5, 10, 10)], T), vec![r(0, 0, 5, 5)]);
        assert!(plan(&[r(200, 200, 10, 10)], T).is_empty());
    }

    #[test]
    fn many_rects_become_one_union() {
        let many: Vec<_> = (0..20).map(|i| r(i * 5, i * 5, 1, 1)).collect();
        assert_eq!(plan(&many, T), vec![r(0, 0, 96, 96)]);
    }
}
