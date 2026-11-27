//! The planes of a page laid out: what is on each measured, placed by the runtime's layout, and written
//! back, pass by pass until nothing changes. A port of @extended-mark/dom's `planes.ts`, shared by the
//! renderers here that draw planes.

use std::collections::HashSet;
use std::rc::Rc;

use xtmd::eval::interp::R;
use xtmd::eval::value::Record;
use xtmd::eval::{Num, Value};
use xtmd_runtime::layout::{lay, Box as OnPlane, Plane, Pt, Region, Size};
use xtmd_runtime::{Drawn, Page, Write};

/// What whoever draws a page tells its layout: the size of each spot's child, and the width a plane is
/// given when it says none.
pub trait Measure {
    fn spot(&mut self, page: &Page, spot: &Drawn) -> Size;
    fn plane_width(&mut self, page: &Page, plane: &Drawn) -> Num;
}

pub fn pt_of(v: Option<&Value>) -> Option<Pt> {
    let Some(Value::Record(r)) = v else { return None };
    match (r.get("x"), r.get("y")) {
        (Some(Value::Num(x)), Some(Value::Num(y))) => Some(Pt { x: *x, y: *y }),
        _ => None,
    }
}

pub fn region_of(v: Option<&Value>) -> Option<Region> {
    match v? {
        Value::Opt(x) => region_of(x.as_deref()),
        Value::Record(r) => Some(Region { from: pt_of(r.get("from"))?, to: pt_of(r.get("to"))? }),
        _ => None,
    }
}

pub fn pt_value(p: &Pt) -> Value {
    Value::Record(Rc::new(Record { ty: Rc::from("core/point"), fields: vec![(Rc::from("x"), Value::Num(p.x)), (Rc::from("y"), Value::Num(p.y))] }))
}

pub fn region_value(r: &Region) -> Value {
    Value::Record(Rc::new(Record { ty: Rc::from("core/region"), fields: vec![(Rc::from("from"), pt_value(&r.from)), (Rc::from("to"), pt_value(&r.to))] }))
}

fn num_of(v: Option<&Value>, or: Num) -> Num {
    match v {
        Some(Value::Num(n)) => *n,
        _ => or,
    }
}

/// Every plane in the tree, outermost first. One that failed has nothing to lay out.
pub fn planes(nodes: &[Drawn]) -> Vec<&Drawn> {
    fn walk<'a>(nodes: &'a [Drawn], out: &mut Vec<&'a Drawn>) {
        for n in nodes {
            if n.widget == "core/plane" && n.failed.is_none() {
                out.push(n);
            }
            for kids in n.slots.values().chain(n.held.iter()) {
                walk(kids, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    out
}

/// A page's planes, laid out: which were already, as `fit` only sets where one starts.
#[derive(Default)]
pub struct Planes {
    started: HashSet<String>,
}

impl Planes {
    /// What one plane writes after a layout: each spot's box, its zoom and its centre when they moved, and
    /// the region it draws.
    fn writes(&mut self, page: &Page, plane: &Drawn, measure: &mut dyn Measure) -> R<Vec<Write>> {
        let zero = Pt { x: Num::zero(), y: Num::zero() };
        let spots: Vec<&Drawn> = plane.slots.get("items").into_iter().flatten().filter(|s| s.widget == "core/spot").collect();
        let boxes = spots
            .iter()
            .map(|s| OnPlane {
                at: pt_of(s.props.get("at")).unwrap_or(zero),
                anchor: match s.props.get("anchor") {
                    Some(Value::Case(c)) => c.name.to_string(),
                    _ => "CENTER".to_string(),
                },
                nudge: pt_of(s.props.get("nudge")).unwrap_or(zero),
                size: measure.spot(page, s),
            })
            .collect();
        let measured = measure.plane_width(page, plane);
        let width = match plane.props.get("width") {
            Some(Value::Opt(Some(w))) => match &**w {
                Value::Case(_) => num_of(Some(&page.token_value(w)?), measured),
                w => num_of(Some(w), measured),
            },
            _ => measured,
        };
        let zoom = num_of(plane.props.get("zoom"), Num::int(1));
        let center = pt_of(plane.props.get("center")).unwrap_or(zero);
        let end = |v: &Option<Value>| match v {
            Some(Value::Num(n)) => Some(*n),
            _ => None,
        };
        let at = lay(&Plane {
            boxes,
            bounds: region_of(plane.props.get("bounds")),
            width,
            aspect: num_of(plane.props.get("aspect"), Num::parse("1.5").unwrap()),
            center,
            zoom,
            fit: matches!(plane.props.get("fit"), Some(Value::Bool(true))),
            first: self.started.insert(plane.id.clone()),
            zoom_range: match plane.props.get("zoom_range") {
                Some(Value::Range(r)) => (end(&r.low), end(&r.high)),
                _ => (None, None),
            },
        })?;
        let write = |id: &str, input: &str, value: Value| Write { id: id.to_string(), input: input.into(), value };
        let mut out: Vec<Write> = spots.iter().zip(&at.boxes).map(|(s, b)| write(&s.id, "box", Value::some(region_value(b)))).collect();
        if at.zoom != zoom {
            out.push(write(&plane.id, "zoom", Value::Num(at.zoom)));
        }
        if at.center.x != center.x || at.center.y != center.y {
            out.push(write(&plane.id, "center", pt_value(&at.center)));
        }
        out.push(write(&plane.id, "drawn", Value::some(region_value(&at.drawn))));
        Ok(out)
    }

    /// Lays the page's planes out.
    pub fn lay(&mut self, page: &mut Page, measure: &mut dyn Measure) -> R<()> {
        page.settle(&mut |page| {
            let all = planes(page.tree());
            let mut writes = Vec::new();
            for p in &all {
                writes.extend(self.writes(page, p, measure)?);
            }
            Ok((writes, all.iter().map(|p| p.id.clone()).collect()))
        })
    }
}
