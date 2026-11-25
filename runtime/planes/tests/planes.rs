//! Planes laid out on the native runtime, with fixed sizes: what a layout writes, one that never settles,
//! and a drag, with and without the page's reuse. The same pages as @extended-mark/dom's planes.test.mjs.

#[path = "../../rust/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use common::*;
use xtmd_planes::{Measure, Planes};
use xtmd_runtime::ir::Expr;
use xtmd_runtime::{Drawn, Num, Page, PageValues, PageWidth, Size};

fn state(p: &Page, name: &str) -> Expr {
    p.observe_state().remove(name).unwrap()
}

const MAP: &str = "$state {\n    home: opt<region> = opt.NONE;\n}\n\nwidget map() {\n    view {\n        @plane(label = \"Map\", bounds = region { from = point { x = -100, y = -100 }, to = point { x = 100, y = 100 } }) {\n            items {\n                @spot(at = point { x = 0, y = 0 }, box = $state.home) {\n                    @label(value = \"Home\");\n                }\n            }\n        }\n    }\n}\n@@@\n@map;\n";

struct Fixed {
    width: Num,
    measured: Rc<Cell<u32>>,
    /// Each spot measured grows by this much: a layout that never settles.
    grow: i64,
}

impl Measure for Fixed {
    fn spot(&mut self, _page: &Page, _spot: &Drawn) -> Size {
        self.measured.set(self.measured.get() + 1);
        let extra = self.grow * self.measured.get() as i64;
        Size { width: Num::int(32 + extra), height: Num::int(20) }
    }

    fn plane_width(&mut self, _page: &Page, _plane: &Drawn) -> Num {
        self.width
    }
}

fn region_expr(fx: &str, fy: &str, tx: &str, ty: &str) -> Expr {
    use xtmd_runtime::ir::ExprRecord;
    let point = |x: &str, y: &str| Expr::Record(ExprRecord { ty: "core/point".into(), fields: BTreeMap::from([("x".to_string(), num(x)), ("y".to_string(), num(y))]) });
    Expr::Some(Box::new(Expr::Record(ExprRecord { ty: "core/region".into(), fields: BTreeMap::from([("from".to_string(), point(fx, fy)), ("to".to_string(), point(tx, ty))]) })))
}

#[test]
fn a_layout_writes_boxes_once_a_second_pass_with_nothing_changed_writes_nothing() {
    let mut p = page(MAP);
    let mut planes = Planes::default();
    let redraws = Rc::new(Cell::new(0));
    let r = redraws.clone();
    p.on_change(move || r.set(r.get() + 1));
    let mut measure = Fixed { width: n("100"), measured: Rc::new(Cell::new(0)), grow: 0 };
    planes.lay(&mut p, &mut measure).unwrap();
    assert_eq!(redraws.get(), 1);
    assert_eq!(state(&p, "home"), region_expr("-16", "-10", "16", "10"));
    // the plane's drawn region, in its units
    let plane = p.observe_tree().into_iter().find(|(_, m)| m.widget == "core/plane").unwrap().1;
    assert!(plane.props.contains_key("drawn"));
    planes.lay(&mut p, &mut measure).unwrap();
    assert_eq!(redraws.get(), 1);
    assert_eq!(p.take_outcome(), Default::default());
}

#[test]
fn a_layout_that_never_settles_fails_its_plane_and_is_held_until_an_outside_change() {
    let mut p = page(MAP);
    let mut planes = Planes::default();
    let measured = Rc::new(Cell::new(0));
    let mut measure = Fixed { width: n("100"), measured: measured.clone(), grow: 2 };
    planes.lay(&mut p, &mut measure).unwrap();
    assert_eq!(measured.get(), xtmd_runtime::MAX_PASSES as u32);
    let failures = p.take_outcome().failures;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].code.as_deref(), Some("failure.layout"));
    // held: laying out again, as an engine does every frame, runs no pass
    planes.lay(&mut p, &mut measure).unwrap();
    planes.lay(&mut p, &mut measure).unwrap();
    assert_eq!(measured.get(), xtmd_runtime::MAX_PASSES as u32);
    // until something outside the layout changes the page
    p.set_page(PageValues { width: Some(PageWidth::Wide), theme: None }).unwrap();
    planes.lay(&mut p, &mut measure).unwrap();
    assert_eq!(measured.get(), 2 * xtmd_runtime::MAX_PASSES as u32);
}

#[test]
fn dragging_on_the_tour_page_observes_the_same_with_and_without_reuse() {
    let source = std::fs::read_to_string(repo().join("renderers/compose/sample/pages/tour.xtmd")).unwrap();
    let point = |x: &str, y: &str| {
        Expr::Record(xtmd_runtime::ir::ExprRecord { ty: "core/point".into(), fields: BTreeMap::from([("x".to_string(), num(x)), ("y".to_string(), num(y))]) })
    };
    let find = |p: &Page, widget: &str| p.observe_tree().into_iter().find(|(_, m)| m.widget == widget).unwrap().0;
    let lay = |p: &mut Page| {
        let mut m = Fixed { width: n("640"), measured: Rc::new(Cell::new(0)), grow: 0 };
        Planes::default().lay(p, &mut m).unwrap();
    };
    let handle = |x: &'static str, y: &'static str| {
        move |p: &mut Page| {
            let at = find(p, "core/handle");
            let value = v(p, point(x, y));
            p.write(&at, "at", value).unwrap();
            lay(p);
        }
    };
    let pan = |x: &'static str, y: &'static str| {
        move |p: &mut Page| {
            let at = find(p, "core/plane");
            let value = v(p, point(x, y));
            p.write(&at, "center", value).unwrap();
            lay(p);
        }
    };
    let (h1, h2, p1, p2, h3) = (handle("0.5", "0.2"), handle("0.6", "0.1"), pan("20", "10"), pan("40", "-20"), handle("-0.3", "0.9"));
    let dark = |p: &mut Page| p.set(vec![("dark".into(), v(p, Expr::Bool(true)))]).unwrap();
    same_with_and_without_reuse(&source, &[&lay, &h1, &h2, &p1, &dark, &p2, &h3]);
}
