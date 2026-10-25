//! Text gamma and contrast parameters for the mask-sprite shader (architecture §8.1, text-stack §3).
//!
//! The atlas holds linear coverage (rasterised with gamma 1, contrast 0); the shader applies the
//! DirectWrite/Windows Terminal correction, which assumes blending in sRGB-encoded space, as this
//! renderer does.

/// What the mask-sprite shader needs to correct glyph coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextParams {
    /// Display gamma, 1.0..=2.2 (DirectWrite's default is 1.8; macOS uses 1.0).
    pub gamma: f32,
    /// Grayscale enhanced contrast, ≥ 0 (DirectWrite's default is 1.0; macOS uses 0).
    pub grayscale_contrast: f32,
}

impl Default for TextParams {
    fn default() -> Self {
        TextParams { gamma: 1.8, grayscale_contrast: 1.0 }
    }
}

impl TextParams {
    pub(crate) fn ratios(&self) -> [f32; 4] {
        gamma_correction_ratios(self.gamma)
    }
}

// Derived from zed-industries/zed crates/gpui/src/platform.rs @ a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc
// (`get_gamma_correction_ratios`), itself adapted from
// https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.cpp
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
// Modifications by the svelters authors.
/// The four gamma ratios the shader's `apply_alpha_correction` takes, for `gamma` in 1.0..=2.2.
pub fn gamma_correction_ratios(gamma: f32) -> [f32; 4] {
    const GAMMA_INCORRECT_TARGET_RATIOS: [[f32; 4]; 13] = [
        [0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0, 0.0000 / 4.0], // gamma = 1.0
        [0.0166 / 4.0, -0.0807 / 4.0, 0.2227 / 4.0, -0.0751 / 4.0], // gamma = 1.1
        [0.0350 / 4.0, -0.1760 / 4.0, 0.4325 / 4.0, -0.1370 / 4.0], // gamma = 1.2
        [0.0543 / 4.0, -0.2821 / 4.0, 0.6302 / 4.0, -0.1876 / 4.0], // gamma = 1.3
        [0.0739 / 4.0, -0.3963 / 4.0, 0.8167 / 4.0, -0.2287 / 4.0], // gamma = 1.4
        [0.0933 / 4.0, -0.5161 / 4.0, 0.9926 / 4.0, -0.2616 / 4.0], // gamma = 1.5
        [0.1121 / 4.0, -0.6395 / 4.0, 1.1588 / 4.0, -0.2877 / 4.0], // gamma = 1.6
        [0.1300 / 4.0, -0.7649 / 4.0, 1.3159 / 4.0, -0.3080 / 4.0], // gamma = 1.7
        [0.1469 / 4.0, -0.8911 / 4.0, 1.4644 / 4.0, -0.3234 / 4.0], // gamma = 1.8
        [0.1627 / 4.0, -1.0170 / 4.0, 1.6051 / 4.0, -0.3347 / 4.0], // gamma = 1.9
        [0.1773 / 4.0, -1.1420 / 4.0, 1.7385 / 4.0, -0.3426 / 4.0], // gamma = 2.0
        [0.1908 / 4.0, -1.2652 / 4.0, 1.8650 / 4.0, -0.3476 / 4.0], // gamma = 2.1
        [0.2031 / 4.0, -1.3864 / 4.0, 1.9851 / 4.0, -0.3501 / 4.0], // gamma = 2.2
    ];

    const NORM13: f32 = ((0x10000 as f64) / (255.0 * 255.0) * 4.0) as f32;
    const NORM24: f32 = ((0x100 as f64) / (255.0) * 4.0) as f32;

    let index = ((gamma * 10.0).round() as usize).clamp(10, 22) - 10;
    let ratios = GAMMA_INCORRECT_TARGET_RATIOS[index];

    [ratios[0] * NORM13, ratios[1] * NORM24, ratios[2] * NORM13, ratios[3] * NORM24]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios_for_gamma_1_8_match_windows_terminal() {
        // text-stack §3: (0.148054421, -0.894594550, 1.47590804, -0.324668258).
        let r = gamma_correction_ratios(1.8);
        let want = [0.148054421, -0.894594550, 1.47590804, -0.324668258];
        for (a, b) in r.iter().zip(want) {
            assert!((a - b).abs() < 1e-5, "{r:?}");
        }
        assert_eq!(gamma_correction_ratios(1.0), [0.0; 4]);
        assert_eq!(gamma_correction_ratios(9.0), gamma_correction_ratios(2.2));
    }
}
