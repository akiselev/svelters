//! Where a table's parts are, in table-local points: the frozen header at the top, the body under
//! it, the shown columns' x ranges (the last one stretched to fill the body with `stretch-last`),
//! the first `sticky` columns fixed at the left and the rest scrolled by `off.x`, rows scrolled by
//! `off.y` under the header.

use presto_base::geom::{Point, Rect, Vec2};

#[derive(Clone, Debug, PartialEq)]
pub struct Geom {
    pub body: Rect,
    pub head: Rect,
    /// Each shown column's content x (from the first column's left edge) and drawn width.
    pub xs: Vec<f32>,
    pub ws: Vec<f32>,
    pub sticky: usize,
    pub sticky_w: f32,
    /// The shown columns' total width.
    pub total: f32,
    /// The scroll offset (content points).
    pub off: Vec2,
    pub row_h: f32,
}

impl Geom {
    pub fn new(r: Rect, header_h: f32, widths: &[f32], sticky: usize, stretch_last: bool, row_h: f32) -> Geom {
        let head = Rect::new(r.min.x, r.min.y, r.max.x, r.min.y + header_h.min(r.height()).max(0.0));
        let body = Rect::new(r.min.x, head.max.y, r.max.x, r.max.y.max(head.max.y));
        let mut ws = widths.to_vec();
        let sum: f32 = ws.iter().sum();
        if stretch_last
            && sum < body.width()
            && let Some(l) = ws.last_mut()
        {
            *l += body.width() - sum;
        }
        let mut xs = Vec::with_capacity(ws.len());
        let mut x = 0.0;
        for w in &ws {
            xs.push(x);
            x += w;
        }
        let sticky = sticky.min(ws.len());
        Geom { body, head, sticky_w: ws[..sticky].iter().sum(), xs, ws, sticky, total: x, off: Vec2::ZERO, row_h: row_h.max(1.0) }
    }

    /// Column `k`'s x range.
    pub fn col_x(&self, k: usize) -> (f32, f32) {
        let x = self.body.min.x + self.xs[k] - if k < self.sticky { 0.0 } else { self.off.x };
        (x, x + self.ws[k])
    }

    /// The x range column `k` may paint in (scrolled columns stay right of the sticky ones).
    pub fn clip_x(&self, k: usize) -> (f32, f32) {
        if k < self.sticky {
            (self.body.min.x, self.body.max.x)
        } else {
            ((self.body.min.x + self.sticky_w).min(self.body.max.x), self.body.max.x)
        }
    }

    /// The columns with something in view.
    pub fn visible_cols(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.ws.len()).filter(|&k| {
            let ((a, b), (l, r)) = (self.col_x(k), self.clip_x(k));
            b > l && a < r
        })
    }

    pub fn col_at(&self, x: f32) -> Option<usize> {
        self.visible_cols().find(|&k| {
            let ((a, b), (l, r)) = (self.col_x(k), self.clip_x(k));
            x >= a.max(l) && x < b.min(r)
        })
    }

    pub fn row_at(&self, y: f32, len: u64) -> Option<u64> {
        let p = ((y - self.body.min.y + self.off.y) / self.row_h).floor();
        (p >= 0.0 && (p as u64) < len && y >= self.body.min.y && y < self.body.max.y).then_some(p as u64)
    }

    /// The rows with something in view: `first..end` among `len`.
    pub fn visible_rows(&self, len: u64) -> (u64, u64) {
        let first = ((self.off.y / self.row_h).floor().max(0.0) as u64).min(len);
        let end = (((self.off.y + self.body.height()) / self.row_h).ceil().max(0.0) as u64).min(len);
        (first, end.max(first))
    }

    /// Row `pos`'s rect (full width of the shown columns, cut at the body's right).
    pub fn row_rect(&self, pos: u64) -> Rect {
        let y = self.body.min.y + pos as f32 * self.row_h - self.off.y;
        let right = (self.body.min.x + self.total - self.off.x).max(self.body.min.x + self.sticky_w).min(self.body.max.x);
        Rect::new(self.body.min.x, y, right, y + self.row_h)
    }

    /// Cell (`pos`, `k`)'s rect and the rect it may paint in.
    pub fn cell_rect(&self, pos: u64, k: usize) -> (Rect, Rect) {
        let y = self.body.min.y + pos as f32 * self.row_h - self.off.y;
        let (a, b) = self.col_x(k);
        let (l, r) = self.clip_x(k);
        (Rect::new(a, y, b, y + self.row_h), Rect::new(l, self.body.min.y, r, self.body.max.y))
    }

    /// Header cell `k`'s rect and the rect it may paint in.
    pub fn head_rect(&self, k: usize) -> (Rect, Rect) {
        let (a, b) = self.col_x(k);
        let (l, r) = self.clip_x(k);
        (Rect::new(a, self.head.min.y, b, self.head.max.y), Rect::new(l, self.head.min.y, r, self.head.max.y))
    }

    /// The shown column whose right edge is within `hit / 2` of `p` in the header (the visible
    /// edge nearest the pointer when two are close).
    pub fn edge_at(&self, p: Point, hit: f32) -> Option<usize> {
        if !self.head.contains(p) {
            return None;
        }
        self.visible_cols()
            .filter_map(|k| {
                let (_, x) = self.col_x(k);
                let (l, r) = self.clip_x(k);
                let d = (p.x - x).abs();
                (x >= l && x <= r + 0.5 && d <= hit / 2.0).then_some((k, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// The offset that brings row `pos` (of `len`) and, past the sticky ones, shown column `col`
    /// into view from `off`.
    pub fn reveal(&self, off: Vec2, pos: Option<u64>, col: Option<usize>, len: u64) -> Vec2 {
        let mut off = off;
        let view = self.body.size();
        if let Some(row) = pos {
            let y = row.min(len.max(1) - 1) as f32 * self.row_h;
            if y < off.y {
                off.y = y;
            } else if y + self.row_h > off.y + view.h {
                off.y = y + self.row_h - view.h;
            }
        }
        if let Some(k) = col.filter(|&k| k < self.ws.len() && k >= self.sticky) {
            let (x0, x1) = (self.xs[k] - self.sticky_w, self.xs[k] + self.ws[k] - self.sticky_w);
            let room = view.w - self.sticky_w;
            if x0 < off.x {
                off.x = x0;
            } else if x1 > off.x + room {
                off.x = (x1 - room).min(x0);
            }
        }
        off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_stretches_the_last_column_and_keeps_sticky_ones() {
        let r = Rect::new(0.0, 0.0, 300.0, 124.0);
        let mut g = Geom::new(r, 24.0, &[100.0, 80.0], 1, true, 20.0);
        assert_eq!((g.ws[1], g.total), (200.0, 300.0), "stretch-last fills the width");
        let g2 = Geom::new(r, 24.0, &[100.0, 80.0], 1, false, 20.0);
        assert_eq!(g2.total, 180.0);
        // scrolled: the sticky column stays, the other moves under it
        g.ws = vec![100.0, 400.0];
        g.total = 500.0;
        g.off = Vec2::new(50.0, 30.0);
        assert_eq!(g.col_x(0), (0.0, 100.0));
        assert_eq!(g.col_x(1), (50.0, 450.0));
        assert_eq!(g.clip_x(1).0, 100.0);
        assert_eq!(g.col_at(80.0), Some(0), "the sticky column covers the scrolled one");
        assert_eq!(g.col_at(120.0), Some(1));
        // rows: 30 points scrolled, row 1 starts 10 below the header
        assert_eq!(g.row_at(24.0 + 5.0, 100), Some(1));
        assert_eq!(g.row_rect(1).min.y, 24.0 - 10.0);
        assert_eq!(g.row_at(10.0, 100), None, "the header is not a row");
        assert_eq!(g.visible_rows(100), (1, 7), "rows 1..7 cover 30..130 of a 100-point body");
    }

    #[test]
    fn edges_and_reveal() {
        let r = Rect::new(0.0, 0.0, 200.0, 100.0);
        let g = Geom::new(r, 20.0, &[50.0, 50.0, 300.0], 1, false, 20.0);
        assert_eq!(g.edge_at(Point::new(51.0, 10.0), 4.0), Some(0));
        assert_eq!(g.edge_at(Point::new(99.0, 10.0), 4.0), Some(1));
        assert_eq!(g.edge_at(Point::new(75.0, 10.0), 4.0), None);
        assert_eq!(g.edge_at(Point::new(51.0, 30.0), 4.0), None, "edges are in the header only");
        let off = g.reveal(Vec2::ZERO, Some(10), Some(2), 100);
        assert_eq!(off.y, 11.0 * 20.0 - 80.0, "row 10's bottom at the body's bottom");
        assert_eq!(off.x, 50.0, "column 2 starts right of the sticky column, cut to its left");
    }
}
