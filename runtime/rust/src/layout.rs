//! Where the boxes on a plane go (page 5, plane): sizes and points in, places out, in the plane's units. It
//! knows no page and no widget, so whoever draws planes asks it with what they measured. A port of
//! @extended-mark/runtime's `layout.ts`.
//!
//! At zoom 1 one unit of the plane is one of the page's own lengths (an egui point, an Android dp), so a box
//! is its size on the page, placed at its point: panning and zooming never change it.

use xtmd::eval::interp::R;
use xtmd::eval::Num;

/// A size on the page, in its own lengths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: Num,
    pub height: Num,
}

/// A point in a plane's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: Num,
    pub y: Num,
}

/// A region in a plane's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub from: Pt,
    pub to: Pt,
}

/// A box on a plane, as it was measured.
#[derive(Clone, Debug)]
pub struct Box {
    pub at: Pt,
    /// Which part of it is on its point: `CENTER`, `N`, `SE`…
    pub anchor: String,
    pub nudge: Pt,
    pub size: Size,
}

/// A plane and what is on it.
#[derive(Clone, Debug)]
pub struct Plane {
    pub boxes: Vec<Box>,
    /// Left out, the region around every box.
    pub bounds: Option<Region>,
    /// Its width on the page, and that over its height.
    pub width: Num,
    pub aspect: Num,
    pub center: Pt,
    pub zoom: Num,
    /// `fit` sets where the zoom starts, so only a plane laid out for the first time starts there.
    pub fit: bool,
    pub first: bool,
    /// How far the zoom may go: its ends, either of them open.
    pub zoom_range: (Option<Num>, Option<Num>),
}

/// Where everything on a plane is.
#[derive(Clone, Debug, PartialEq)]
pub struct Places {
    pub boxes: Vec<Region>,
    pub center: Pt,
    pub zoom: Num,
    /// The region it draws.
    pub drawn: Region,
}

fn half() -> Num {
    Num::parse("0.5").unwrap()
}

/// Where a spot's child lands: the anchor's part of it on the point, moved by the nudge.
pub fn place(at: Pt, anchor: &str, nudge: Pt, size: Size) -> R<Region> {
    // how much of the width is left of the point, and of the height below it
    let left = if anchor.ends_with('W') {
        Num::zero()
    } else if anchor.ends_with('E') {
        size.width
    } else {
        size.width.mul(half())?
    };
    let below = if anchor.starts_with('S') {
        Num::zero()
    } else if anchor.starts_with('N') {
        size.height
    } else {
        size.height.mul(half())?
    };
    let x = at.x.add(nudge.x)?.sub(left)?;
    let y = at.y.add(nudge.y)?.sub(below)?;
    Ok(Region { from: Pt { x, y }, to: Pt { x: x.add(size.width)?, y: y.add(size.height)? } })
}

/// The region around every box.
pub fn around(boxes: &[Region]) -> Option<Region> {
    let min = |a: Num, b: Num| if a < b { a } else { b };
    let max = |a: Num, b: Num| if a > b { a } else { b };
    let mut it = boxes.iter();
    let first = *it.next()?;
    Some(it.fold(first, |r, b| Region {
        from: Pt { x: min(r.from.x, b.from.x), y: min(r.from.y, b.from.y) },
        to: Pt { x: max(r.to.x, b.to.x), y: max(r.to.y, b.to.y) },
    }))
}

/// A centre kept where the drawn area stays inside the bounds, on each axis; their middle where
/// the bounds are smaller than it.
pub fn keep_inside(center: Pt, half_size: Pt, bounds: Region) -> R<Pt> {
    let axis = |c: Num, h: Num, lo: Num, hi: Num| -> R<Num> {
        if hi.sub(lo)? < h.add(h)? {
            return Ok(lo.add(hi)?.mul(half())?);
        }
        if c.sub(h)? < lo {
            return Ok(lo.add(h)?);
        }
        if c.add(h)? > hi {
            return Ok(hi.sub(h)?);
        }
        Ok(c)
    };
    Ok(Pt { x: axis(center.x, half_size.x, bounds.from.x, bounds.to.x)?, y: axis(center.y, half_size.y, bounds.from.y, bounds.to.y)? })
}

/// A plane laid out: its boxes, its zoom inside what it allows, its centre kept where the drawn area stays
/// inside the bounds, and the region it draws.
pub fn lay(p: &Plane) -> R<Places> {
    let mut boxes = Vec::new();
    for b in &p.boxes {
        boxes.push(place(b.at, &b.anchor, b.nudge, b.size)?);
    }
    let bounds = p.bounds.or_else(|| around(&boxes));
    // `fit` starts the zoom where the whole of the bounds fits, and the zoom's range counts from there
    let mut base = Num::int(1);
    if let (true, Some(b)) = (p.fit, bounds) {
        let bw = b.to.x.sub(b.from.x)?;
        let bh = b.to.y.sub(b.from.y)?;
        if !bw.is_zero() && !bh.is_zero() {
            let across = p.width.div(bw)?;
            let up = p.width.div(p.aspect)?.div(bh)?;
            base = if across < up { across } else { up };
        }
    }
    let mut zoom = if p.first && p.fit { base } else { p.zoom };
    if let Some(lo) = p.zoom_range.0 {
        let lo = lo.mul(base)?;
        if zoom < lo {
            zoom = lo;
        }
    }
    if let Some(hi) = p.zoom_range.1 {
        let hi = hi.mul(base)?;
        if zoom > hi {
            zoom = hi;
        }
    }
    let (least, most) = (Num::parse("0.1").unwrap(), Num::int(10));
    if zoom < least {
        zoom = least;
    }
    if zoom > most {
        zoom = most;
    }
    // the drawn area, in the plane's units: the widget's size on the page over the zoom
    let across = Pt { x: p.width.div(zoom)?.mul(half())?, y: p.width.div(p.aspect)?.div(zoom)?.mul(half())? };
    let center = match bounds {
        Some(b) => keep_inside(p.center, across, b)?,
        None => p.center,
    };
    let drawn = Region { from: Pt { x: center.x.sub(across.x)?, y: center.y.sub(across.y)? }, to: Pt { x: center.x.add(across.x)?, y: center.y.add(across.y)? } };
    Ok(Places { boxes, center, zoom, drawn })
}
