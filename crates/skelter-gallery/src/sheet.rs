//! Contact sheets: one PNG of a story's matrix plus an index of where each cell landed. Rows are
//! variant × args × state (the matrix's order), column groups are theme, then zoom, then density.
//! Each cell has a small header with its ids. Labels are drawn with skelter-text's layout and
//! rasteriser over the renderer's (bundled, hermetic) fonts.

use crate::matrix::Matrix;
use crate::model::{Cell, Theme};
use crate::render::{Error, Renderer};
use serde::Serialize;
use skelter_base::text::{GlyphFormat, GlyphKey, GlyphRasterizer};
use skelter_test::Image;
use skelter_text::{FontRasterizer, FontStore, RichText, TextStyle, TextSystem};
use skelter_widgets::theme::Density;
use std::sync::Arc;
use std::time::Instant;

/// How a sheet is laid out (pixels of the sheet image).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SheetLayout {
    /// Space between cells and around the sheet.
    pub gap: u32,
    /// The label font size (pixels).
    pub font_size: f32,
    /// The narrowest a column gets (labels need room even for a 24 px icon).
    pub min_column: u32,
    pub background: [u8; 4],
    pub text: [u8; 4],
    pub error: [u8; 4],
}

impl Default for SheetLayout {
    fn default() -> SheetLayout {
        SheetLayout { gap: 8, font_size: 11.0, min_column: 140, background: [0x26, 0x26, 0x2a, 0xff], text: [0xd0, 0xd0, 0xd0, 0xff], error: [0xff, 0x6b, 0x6b, 0xff] }
    }
}

/// A pixel rectangle of the sheet image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PxRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Where one cell landed.
#[derive(Clone, Debug, Serialize)]
pub struct SheetCell {
    pub cell: Cell,
    /// The header text, as drawn.
    pub label: String,
    /// The cell's image (its device pixels, story size × zoom).
    pub rect: PxRect,
    /// Its header above the image.
    pub header: PxRect,
    /// How the state was applied (`forced :hover on 1 subject`, `disabled prop`, ...).
    pub applied: Option<String>,
    pub warnings: Vec<String>,
    /// Why the cell has no image (it failed to load or threw).
    pub error: Option<String>,
}

/// A rendered sheet.
#[derive(Clone, Debug)]
pub struct Sheet {
    /// Opaque RGBA8.
    pub image: Image,
    pub cells: Vec<SheetCell>,
    /// Rendering time, for cells per second.
    pub seconds: f64,
}

impl Sheet {
    /// Cells that failed.
    pub fn errors(&self) -> impl Iterator<Item = &SheetCell> {
        self.cells.iter().filter(|c| c.error.is_some())
    }

    /// The index as JSON (the `--index` file's entry for one sheet).
    pub fn index(&self, matrix: &Matrix, file: Option<&std::path::Path>) -> serde_json::Value {
        serde_json::json!({
            "story": matrix.story.id,
            "title": matrix.story.title,
            "file": file,
            "size": [self.image.width, self.image.height],
            "cells": self.cells,
            "seconds": self.seconds,
        })
    }
}

/// Text drawing for the labels.
pub(crate) struct Labels {
    text: TextSystem,
    raster: FontRasterizer,
}

impl Labels {
    pub(crate) fn new(fonts: Arc<FontStore>) -> Labels {
        Labels { text: TextSystem::new(fonts.clone()), raster: FontRasterizer::new(fonts) }
    }

    /// Draws `s` with its top-left at (`x`, `y`), clipped to `clip_w` pixels.
    pub(crate) fn draw(&mut self, img: &mut Image, s: &str, x: u32, y: u32, clip_w: u32, size: f32, color: [u8; 4]) {
        let l = self.text.layout(&RichText::plain(s, TextStyle { size, wrap: false, ..TextStyle::default() }), None);
        for pr in l.runs() {
            let r = &pr.run;
            for g in &r.glyphs {
                let gx = x as f32 + pr.origin.x + g.x;
                let key = GlyphKey {
                    font: r.font,
                    variations: r.variations,
                    glyph: g.id,
                    ppem_26_6: (r.size * 64.0).round() as u32,
                    x_bin: ((gx.fract() * 4.0) as u8).min(3),
                    hint: r.hint,
                    aa: r.aa,
                    dilation: 0,
                    embolden_64: (r.embolden * 64.0).round() as u16,
                    skew_deg: r.skew_deg.round() as i8,
                };
                let Some(gi) = self.raster.rasterize(&key) else { continue };
                let ox = gx.floor() as i32 + gi.left;
                let oy = (y as f32 + pr.origin.y + g.y).round() as i32 + gi.top;
                for row in 0..gi.height as i32 {
                    for col in 0..gi.width as i32 {
                        let (px, py) = (ox + col, oy + row);
                        if px < x as i32 || px >= (x + clip_w) as i32 || px < 0 || py < 0 || px >= img.width as i32 || py >= img.height as i32 {
                            continue;
                        }
                        let i = (row as u32 * gi.width + col as u32) as usize;
                        let a = match gi.format {
                            GlyphFormat::Mask => gi.data[i],
                            GlyphFormat::Color => gi.data[i * 4 + 3],
                        };
                        if a > 0 {
                            blend(img, px as u32, py as u32, color, a);
                        }
                    }
                }
            }
        }
    }
}

fn blend(img: &mut Image, x: u32, y: u32, c: [u8; 4], a: u8) {
    let i = ((y * img.width + x) * 4) as usize;
    let a = u32::from(a) * u32::from(c[3]) / 255;
    for k in 0..3 {
        let d = u32::from(img.rgba[i + k]);
        img.rgba[i + k] = ((u32::from(c[k]) * a + d * (255 - a) + 127) / 255) as u8;
    }
    img.rgba[i + 3] = 255;
}

pub(crate) fn fill(img: &mut Image, r: PxRect, c: [u8; 4]) {
    for y in r.y..(r.y + r.h).min(img.height) {
        for x in r.x..(r.x + r.w).min(img.width) {
            let i = ((y * img.width + x) * 4) as usize;
            img.rgba[i..i + 4].copy_from_slice(&c);
        }
    }
}

/// Composites premultiplied `src` over `img` at (`x`, `y`).
pub(crate) fn paste(img: &mut Image, src: &Image, x: u32, y: u32) {
    for sy in 0..src.height {
        let dy = y + sy;
        if dy >= img.height {
            break;
        }
        for sx in 0..src.width {
            let dx = x + sx;
            if dx >= img.width {
                break;
            }
            let s = ((sy * src.width + sx) * 4) as usize;
            let d = ((dy * img.width + dx) * 4) as usize;
            let a = u32::from(src.rgba[s + 3]);
            for k in 0..3 {
                let v = u32::from(src.rgba[s + k]) + u32::from(img.rgba[d + k]) * (255 - a) / 255;
                img.rgba[d + k] = v.min(255) as u8;
            }
            img.rgba[d + 3] = 255;
        }
    }
}

/// The header lines of a cell: `variant args` and `state theme [density] [xZ]`.
fn header_lines(c: &Cell, applied: Option<&str>) -> [String; 2] {
    let mut a = c.variant.0.clone();
    let args = c.args_label();
    if !args.is_empty() {
        a.push_str(&format!(" {args}"));
    }
    let forced = applied.is_some_and(|x| x.starts_with("forced :disabled"));
    let mut b = format!("{}{} {}", c.state, if forced { " (forced bit)" } else { "" }, c.theme);
    if c.density != Density::Default {
        b.push_str(&format!(" {}", c.density.name()));
    }
    if c.zoom != 1.0 {
        b.push_str(&format!(" x{}", c.zoom));
    }
    [a, b]
}

impl Renderer {
    /// Renders every cell of `matrix` into one sheet. A cell that fails (its variant doesn't load or
    /// throws) is drawn as its error and listed with it; the rest render.
    pub fn sheet(&mut self, matrix: &Matrix, layout: SheetLayout) -> Result<Sheet, Error> {
        let t0 = Instant::now();
        // rows: (variant, args, state) in order; columns: (theme, zoom, density)
        type ColKey = (usize, usize, usize);
        let mut rows: Vec<(String, String, String)> = Vec::new();
        let (mut themes, mut zooms, mut dens): (Vec<Theme>, Vec<f32>, Vec<Density>) = (Vec::new(), Vec::new(), Vec::new());
        for c in &matrix.cells {
            let rk = (c.variant.0.clone(), c.args_label(), c.state.name().to_string());
            if !rows.contains(&rk) {
                rows.push(rk);
            }
            if !themes.contains(&c.theme) {
                themes.push(c.theme.clone());
            }
            if !zooms.contains(&c.zoom) {
                zooms.push(c.zoom);
            }
            if !dens.contains(&c.density) {
                dens.push(c.density);
            }
        }
        let key = |c: &Cell| -> ColKey {
            (themes.iter().position(|t| *t == c.theme).unwrap_or(0), zooms.iter().position(|z| *z == c.zoom).unwrap_or(0), dens.iter().position(|d| *d == c.density).unwrap_or(0))
        };
        let mut cols: Vec<ColKey> = matrix.cells.iter().map(key).collect();
        cols.sort();
        cols.dedup();
        let [sw, sh] = matrix.story.size;
        let g = layout.gap;
        let line = (layout.font_size * 1.4).ceil() as u32;
        let header_h = line * 2 + 2;
        let title_h = line + g;
        let col_w: Vec<u32> = cols.iter().map(|&(_, z, _)| ((sw as f32 * zooms[z]).round() as u32).max(layout.min_column)).collect();
        let row_h = header_h + (sh as f32 * zooms.iter().cloned().fold(1.0, f32::max)).round() as u32;
        let width = g + col_w.iter().map(|w| w + g).sum::<u32>();
        let height = g + title_h + rows.len() as u32 * (row_h + g);
        let mut img = Image { width, height, rgba: vec![0; (width * height * 4) as usize] };
        fill(&mut img, PxRect { x: 0, y: 0, w: width, h: height }, layout.background);
        let mut labels = Labels::new(self.fonts().clone());
        let title = format!("{} ({}, {})  {} cells", matrix.story.title, matrix.story.id, matrix.story.family, matrix.cells.len());
        labels.draw(&mut img, &title, g, g, width.saturating_sub(2 * g), layout.font_size + 1.0, layout.text);
        let mut out = Vec::new();
        for c in &matrix.cells {
            let r = rows.iter().position(|rk| *rk == (c.variant.0.clone(), c.args_label(), c.state.name().to_string())).unwrap_or(0);
            let col = cols.iter().position(|k| *k == key(c)).unwrap_or(0);
            let x = g + col_w[..col].iter().map(|w| w + g).sum::<u32>();
            let y = g + title_h + r as u32 * (row_h + g);
            let (iw, ih) = ((sw as f32 * c.zoom).round() as u32, (sh as f32 * c.zoom).round() as u32);
            let header = PxRect { x, y, w: col_w[col], h: header_h };
            let rect = PxRect { x, y: y + header_h, w: iw, h: ih };
            let (applied, warnings, error) = match self.shot(c) {
                Ok(s) => {
                    paste(&mut img, &s.image, rect.x, rect.y);
                    (s.applied, s.warnings, None)
                }
                Err(e) => {
                    let msg = e.to_string();
                    fill(&mut img, rect, [0x40, 0x10, 0x10, 0xff]);
                    for (i, chunk) in wrap(&msg, (iw.max(layout.min_column) as f32 / (layout.font_size * 0.55)) as usize).iter().take(((ih / line).max(1)) as usize).enumerate() {
                        labels.draw(&mut img, chunk, rect.x + 2, rect.y + 2 + i as u32 * line, col_w[col] - 4, layout.font_size, layout.error);
                    }
                    (None, Vec::new(), Some(msg))
                }
            };
            let lines = header_lines(c, applied.as_deref());
            let color = if error.is_some() { layout.error } else { layout.text };
            labels.draw(&mut img, &lines[0], x, y, col_w[col], layout.font_size, color);
            labels.draw(&mut img, &lines[1], x, y + line, col_w[col], layout.font_size, color);
            out.push(SheetCell { cell: c.clone(), label: lines.join(" / "), rect, header, applied, warnings, error });
        }
        Ok(Sheet { image: img, cells: out, seconds: t0.elapsed().as_secs_f64() })
    }
}

/// `s` in lines of at most `n` characters, at spaces where it can.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let n = n.max(8);
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > n {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
