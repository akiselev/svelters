//! Texture atlases (architecture §8.4), CPU side: tile allocation with etagere, the key → tile map,
//! pending uploads and frame-generation LRU eviction. The renderer mirrors each page in a GPU texture,
//! recreating it when the page's generation changes.
//!
//! A page is 1024² unless a tile needs more (up to 2048²). A tile larger than the largest page gets a
//! page of its own sized to it. When every page is full and the page budget is spent, the page used
//! least recently (and not in the current frame) is cleared and reused; pages idle for
//! [`IDLE_FRAMES`] are freed at the end of a frame.

use std::collections::HashMap;

use etagere::{AllocId, BucketedAtlasAllocator, size2};
use presto_base::ids::ImageId;
use presto_base::text::GlyphKey;

pub const DEFAULT_PAGE: u32 = 1024;
pub const MAX_PAGE: u32 = 2048;
/// Pages not touched for this many frames are freed (one is always kept).
pub const IDLE_FRAMES: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasKind {
    /// R8 coverage.
    Mask,
    /// RGBA8 premultiplied.
    Color,
}

impl AtlasKind {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            AtlasKind::Mask => 1,
            AtlasKind::Color => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtlasKey {
    Glyph(GlyphKey),
    /// An image or icon from [`presto_base::scene::SceneResources`] (`mask` = stored as coverage).
    Image { id: ImageId, mask: bool },
}

/// Where a tile's pixels are: `page` index and the texel rect (padding excluded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRef {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug)]
struct Tile {
    tile: TileRef,
    alloc: AllocId,
}

struct Page {
    alloc: BucketedAtlasAllocator,
    size: u32,
    generation: u64,
    last_used: u64,
    keys: Vec<AtlasKey>,
}

/// One texel rect to write into a page (tightly packed rows).
#[derive(Debug)]
pub struct Upload {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

/// A page the GPU should have: when `generation` changes, recreate its texture at `size`².
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageInfo {
    pub index: u32,
    pub size: u32,
    pub generation: u64,
}

pub struct Atlas {
    kind: AtlasKind,
    pages: Vec<Option<Page>>,
    tiles: HashMap<AtlasKey, Tile>,
    pending: Vec<Upload>,
    frame: u64,
    next_generation: u64,
    /// Budget of shared pages (pages of their own for oversized tiles don't count).
    max_pages: usize,
    /// The device's `max_texture_dimension_2d`.
    max_dim: u32,
    /// Bytes queued for upload since the last [`Atlas::take_uploads`].
    pub upload_bytes: u64,
}

impl Atlas {
    pub fn new(kind: AtlasKind, max_pages: usize, max_dim: u32) -> Self {
        Atlas {
            kind,
            pages: Vec::new(),
            tiles: HashMap::new(),
            pending: Vec::new(),
            frame: 1,
            next_generation: 1,
            max_pages: max_pages.max(1),
            max_dim,
            upload_bytes: 0,
        }
    }

    /// The tile for `key`, marked used this frame.
    pub fn get(&mut self, key: &AtlasKey) -> Option<TileRef> {
        let tile = self.tiles.get(key)?.tile;
        if let Some(Some(page)) = self.pages.get_mut(tile.page as usize) {
            page.last_used = self.frame;
        }
        Some(tile)
    }

    /// Allocates a tile for `key` and queues `data` (`w`×`h`, tightly packed) with `pad` transparent
    /// texels around it. `None` when it can't fit even after eviction (the caller skips the draw).
    pub fn insert(&mut self, key: AtlasKey, w: u32, h: u32, data: &[u8], pad: u32) -> Option<TileRef> {
        debug_assert_eq!(data.len(), (w * h) as usize * self.kind.bytes_per_pixel());
        if let Some(t) = self.get(&key) {
            return Some(t);
        }
        let (pw, ph) = (w + 2 * pad, h + 2 * pad);
        if pw > self.max_dim || ph > self.max_dim || w == 0 || h == 0 {
            return None;
        }
        let (page_idx, alloc) = self.allocate(pw, ph)?;
        let r = alloc.rectangle;
        let tile = TileRef { page: page_idx, x: r.min.x as u32 + pad, y: r.min.y as u32 + pad, w, h };
        let page = self.pages[page_idx as usize].as_mut().expect("allocated page exists");
        page.keys.push(key);
        page.last_used = self.frame;
        self.tiles.insert(key, Tile { tile, alloc: alloc.id });

        let bpp = self.kind.bytes_per_pixel();
        let data = if pad == 0 {
            data.to_vec()
        } else {
            let mut out = vec![0u8; (pw * ph) as usize * bpp];
            for row in 0..h as usize {
                let dst = ((row + pad as usize) * pw as usize + pad as usize) * bpp;
                out[dst..dst + w as usize * bpp].copy_from_slice(&data[row * w as usize * bpp..][..w as usize * bpp]);
            }
            out
        };
        self.upload_bytes += data.len() as u64;
        self.pending.push(Upload { page: page_idx, x: r.min.x as u32, y: r.min.y as u32, w: pw, h: ph, data });
        Some(tile)
    }

    fn allocate(&mut self, pw: u32, ph: u32) -> Option<(u32, etagere::Allocation)> {
        let largest = MAX_PAGE.min(self.max_dim);
        if pw > largest || ph > largest {
            // A page of its own, outside the budget.
            let idx = self.new_page(pw.max(ph));
            let alloc = self.pages[idx].as_mut()?.alloc.allocate(size2(pw as i32, ph as i32))?;
            return Some((idx as u32, alloc));
        }
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(page) = page
                && let Some(a) = page.alloc.allocate(size2(pw as i32, ph as i32))
            {
                return Some((i as u32, a));
            }
        }
        let want = DEFAULT_PAGE.max(pw.max(ph).next_power_of_two()).min(largest);
        if self.shared_pages() < self.max_pages {
            let idx = self.new_page(want);
            let a = self.pages[idx].as_mut()?.alloc.allocate(size2(pw as i32, ph as i32))?;
            return Some((idx as u32, a));
        }
        // Evict the least recently used shared page not used this frame, then retry in it.
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
            .filter(|(_, p)| p.last_used < self.frame && p.size <= largest)
            .min_by_key(|(_, p)| p.last_used)
            .map(|(i, _)| i)?;
        self.free_page(victim);
        let idx = self.new_page(want);
        let a = self.pages[idx].as_mut()?.alloc.allocate(size2(pw as i32, ph as i32))?;
        Some((idx as u32, a))
    }

    fn shared_pages(&self) -> usize {
        let largest = MAX_PAGE.min(self.max_dim);
        self.pages.iter().flatten().filter(|p| p.size <= largest).count()
    }

    fn new_page(&mut self, size: u32) -> usize {
        let page = Page {
            alloc: BucketedAtlasAllocator::new(size2(size as i32, size as i32)),
            size,
            generation: self.next_generation,
            last_used: self.frame,
            keys: Vec::new(),
        };
        self.next_generation += 1;
        match self.pages.iter().position(Option::is_none) {
            Some(i) => {
                self.pages[i] = Some(page);
                i
            }
            None => {
                self.pages.push(Some(page));
                self.pages.len() - 1
            }
        }
    }

    fn free_page(&mut self, idx: usize) {
        if let Some(page) = self.pages[idx].take() {
            for key in page.keys {
                self.tiles.remove(&key);
            }
        }
        self.pending.retain(|u| u.page as usize != idx);
    }

    /// Removes one entry (an image the compositor dropped). Its texels are reused by later inserts.
    pub fn remove(&mut self, key: &AtlasKey) {
        if let Some(tile) = self.tiles.remove(key)
            && let Some(Some(page)) = self.pages.get_mut(tile.tile.page as usize)
        {
            page.alloc.deallocate(tile.alloc);
            page.keys.retain(|k| k != key);
        }
    }

    /// Drops every tile (device loss, or a text-parameter change that invalidates rasters).
    pub fn clear(&mut self) {
        self.pages.clear();
        self.tiles.clear();
        self.pending.clear();
    }

    pub fn take_uploads(&mut self) -> Vec<Upload> {
        self.upload_bytes = 0;
        std::mem::take(&mut self.pending)
    }

    pub fn pages(&self) -> impl Iterator<Item = PageInfo> + '_ {
        self.pages.iter().enumerate().filter_map(|(i, p)| {
            p.as_ref().map(|p| PageInfo { index: i as u32, size: p.size, generation: p.generation })
        })
    }

    pub fn page_count(&self) -> usize {
        self.pages.iter().flatten().count()
    }

    /// Advances the frame generation and frees pages idle for [`IDLE_FRAMES`] (keeping one).
    pub fn end_frame(&mut self) {
        let idle: Vec<usize> = self
            .pages
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
            .filter(|(_, p)| self.frame.saturating_sub(p.last_used) >= IDLE_FRAMES)
            .map(|(i, _)| i)
            .collect();
        let mut alive = self.page_count();
        for i in idle {
            if alive <= 1 {
                break;
            }
            self.free_page(i);
            alive -= 1;
        }
        self.frame += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::ids::FontId;
    use presto_base::text::{AaMode, HintMode};

    fn glyph(id: u32) -> AtlasKey {
        AtlasKey::Glyph(GlyphKey {
            font: FontId(1),
            variations: 0,
            glyph: id,
            ppem_26_6: 13 * 64,
            x_bin: 0,
            hint: HintMode::Vertical,
            aa: AaMode::Grayscale,
            dilation: 0,
            embolden_64: 0,
            skew_deg: 0,
        })
    }

    #[test]
    fn insert_get_and_pad() {
        let mut a = Atlas::new(AtlasKind::Mask, 2, 8192);
        let t = a.insert(glyph(1), 3, 2, &[1, 2, 3, 4, 5, 6], 1).unwrap();
        assert_eq!((t.w, t.h), (3, 2));
        assert_eq!(a.get(&glyph(1)), Some(t));
        let ups = a.take_uploads();
        assert_eq!(ups.len(), 1);
        assert_eq!((ups[0].w, ups[0].h), (5, 4));
        assert_eq!(&ups[0].data[5..10], &[0, 1, 2, 3, 0]);
        assert_eq!(a.page_count(), 1);
        assert_eq!(a.pages().next().unwrap().size, DEFAULT_PAGE);
    }

    #[test]
    fn lru_page_is_evicted_when_budget_is_spent() {
        let mut a = Atlas::new(AtlasKind::Mask, 2, 8192);
        let big = vec![0u8; 1000 * 1000];
        a.insert(glyph(1), 1000, 1000, &big, 0).unwrap();
        a.end_frame();
        a.insert(glyph(2), 1000, 1000, &big, 0).unwrap();
        a.end_frame();
        a.get(&glyph(2));
        // Both pages are full; page of glyph 1 is least recently used.
        a.insert(glyph(3), 1000, 1000, &big, 0).unwrap();
        assert!(a.get(&glyph(1)).is_none());
        assert!(a.get(&glyph(2)).is_some());
        assert_eq!(a.page_count(), 2);
    }

    #[test]
    fn pages_used_this_frame_are_never_evicted() {
        let mut a = Atlas::new(AtlasKind::Mask, 1, 8192);
        let big = vec![0u8; 1000 * 1000];
        a.insert(glyph(1), 1000, 1000, &big, 0).unwrap();
        assert!(a.insert(glyph(2), 1000, 1000, &big, 0).is_none());
        assert!(a.get(&glyph(1)).is_some());
    }

    #[test]
    fn oversized_tile_gets_its_own_page_and_idle_pages_are_freed() {
        let mut a = Atlas::new(AtlasKind::Color, 1, 8192);
        a.insert(glyph(1), 2, 2, &[0; 16], 0).unwrap();
        let huge = vec![0u8; 3000 * 10 * 4];
        let t = a.insert(glyph(2), 3000, 10, &huge, 0).unwrap();
        assert_ne!(t.page, 0);
        assert_eq!(a.page_count(), 2);
        for _ in 0..IDLE_FRAMES + 1 {
            a.end_frame();
        }
        assert_eq!(a.page_count(), 1);
    }
}
