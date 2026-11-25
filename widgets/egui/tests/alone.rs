//! The widget set without a page: nodes made by hand, and a host that only notes what it was asked.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use xtmd_egui_widgets::{Act, Contract, Ctx, Drawing, Host, Props, Registry};
use xtmd_runtime::{Drawn, DrawnNode, Num, Value};

/// What a page would be.
#[derive(Clone, Default)]
struct Noted {
    did: Rc<RefCell<Vec<String>>>,
    contract: Contract,
    /// A write is refused.
    refuses: bool,
}

impl Host for Noted {
    fn event(&mut self, id: &str, name: &str, _args: Vec<Value>) -> Result<(), String> {
        self.did.borrow_mut().push(format!("{} {}", id, name));
        Ok(())
    }

    fn write(&mut self, id: &str, input: &str, value: Value) -> Result<bool, String> {
        self.did.borrow_mut().push(format!("{}.{} = {}", id, input, xtmd_egui_widgets::values::text(Some(&value))));
        if self.refuses {
            return Err("no".into());
        }
        Ok(true)
    }

    fn listens(&self, _d: &Drawn, event: &str) -> bool {
        event == "click"
    }

    fn contract(&self, _d: &Drawn, _input: &str) -> Contract {
        self.contract
    }

    fn apply(&self, _f: &Value, _args: Vec<Value>) -> Result<Option<Value>, String> {
        Ok(None)
    }

    fn token(&self, _v: &Value) -> Option<Value> {
        None
    }

    fn equal(&self, _a: &Value, _b: &Value) -> bool {
        false
    }
}

fn node(id: &str, widget: &str, props: &[(&str, Value)], slots: &[(&str, Vec<Drawn>)]) -> Drawn {
    failing(id, widget, props, slots, None)
}

fn failing(id: &str, widget: &str, props: &[(&str, Value)], slots: &[(&str, Vec<Drawn>)], failed: Option<&str>) -> Drawn {
    Drawn::new(DrawnNode {
        path: id.into(),
        id: id.into(),
        widget: widget.into(),
        props: props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        slots: slots.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        events: BTreeMap::new(),
        writes: BTreeMap::new(),
        levels: Vec::new(),
        failed: failed.map(str::to_string),
        held: Vec::new(),
    })
}

fn words(s: &str) -> Value {
    Value::Text(s.into())
}

fn label(id: &str) -> Drawn {
    node(id, "core/label", &[("value", words(id))], &[])
}

/// What is drawn every frame: the widget set, a host, and nodes.
struct Shown {
    drawing: Drawing,
    host: Noted,
    nodes: Vec<Drawn>,
}

fn show(registry: Registry, host: Noted, nodes: Vec<Drawn>) -> Harness<'static, Shown> {
    let shown = Shown { drawing: Drawing::new(registry, egui::Id::new("alone")), host, nodes };
    let mut h = Harness::builder().with_size(egui::vec2(400.0, 400.0)).build_ui_state(
        |ui, s: &mut Shown| {
            let nodes = s.nodes.clone();
            s.drawing.show(&mut s.host, ui, &nodes);
        },
        shown,
    );
    h.run();
    h
}

#[test]
fn an_event_goes_to_whoever_draws() {
    let host = Noted::default();
    let press = node("p", "core/press", &[("label", words("Go"))], &[]);
    let mut h = show(Registry::ours(), host.clone(), vec![press]);
    h.get_by_role_and_label(Role::Button, "Go").click();
    h.run();
    assert_eq!(*host.did.borrow(), ["p click"]);
}

#[test]
fn a_slider_keeps_to_its_variable_s_contract() {
    let host = Noted { contract: Contract { low: Some(Num::int(2)), high: Some(Num::int(8)), step: Some(Num::int(1)) }, ..Noted::default() };
    let slider = node("s", "core/slider", &[("label", words("Size")), ("value", Value::Num(Num::int(5)))], &[]);
    let mut h = show(Registry::ours(), host.clone(), vec![slider]);
    let at = h.get_by_role(Role::Slider).accesskit_node();
    assert_eq!((at.min_numeric_value(), at.max_numeric_value(), at.numeric_value()), (Some(2.0), Some(8.0), Some(5.0)));
    h.get_by_role(Role::Slider).focus();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert_eq!(*host.did.borrow(), ["s.value = 6"]);
}

#[test]
fn a_refused_write_is_told_and_the_rest_is_drawn() {
    let host = Noted { refuses: true, ..Noted::default() };
    let slider = node("s", "core/slider", &[("label", words("Size")), ("value", Value::Num(Num::int(0)))], &[]);
    let mut h = show(Registry::ours(), host.clone(), vec![slider, label("after")]);
    h.get_by_role(Role::Slider).focus();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert_eq!(host.did.borrow().len(), 1);
    assert_eq!(h.state_mut().drawing.take_errors(), ["s value: no"]);
    h.get_by_label("after");
}

#[test]
fn a_widget_replaced_by_its_full_name_is_drawn_inside_ours() {
    let mut r = Registry::ours();
    r.set("core/label", || {
        |ui: &mut egui::Ui, p: &Props<'_>, _: &mut Ctx<'_>| {
            ui.label(format!("mine: {}", p.text("value")));
        }
    });
    let quote = node("q", "core/quote", &[], &[("body", vec![label("hi")])]);
    let h = show(r, Noted::default(), vec![quote]);
    h.get_by_label("mine: hi");
    assert!(h.query_by_label("hi").is_none());
}

#[test]
fn one_that_failed_and_one_nobody_draws_say_so_in_their_place() {
    let nodes = vec![failing("a", "core/label", &[], &[], Some("too deep")), node("b", "acme/none", &[], &[]), label("c")];
    let h = show(Registry::ours(), Noted::default(), nodes);
    assert!(h.query_all_by_label_contains("too deep").next().is_some());
    assert!(h.query_all_by_label_contains("Nothing draws acme/none").next().is_some());
    h.get_by_label("c");
}

#[test]
fn an_act_no_widget_does_is_left_to_whoever_draws() {
    let mut h = show(Registry::ours(), Noted::default(), vec![label("a")]);
    h.state_mut().drawing.act("a", Act::Event { name: "click".into(), args: Vec::new() });
    h.run();
    let left: Vec<String> = h.state_mut().drawing.take_left().into_iter().map(|(path, _)| path).collect();
    assert_eq!(left, ["a"]);
}

#[test]
fn a_widget_s_own_state_leaves_with_it() {
    let made = Rc::new(RefCell::new(0));
    let mut r = Registry::empty();
    let n = made.clone();
    r.set("acme/one", move || {
        *n.borrow_mut() += 1;
        |_: &mut egui::Ui, _: &Props<'_>, _: &mut Ctx<'_>| {}
    });
    let one = node("o", "acme/one", &[], &[]);
    let mut h = show(r, Noted::default(), vec![one.clone()]);
    h.run();
    assert_eq!(*made.borrow(), 1, "kept while it is on the page");
    h.state_mut().nodes.clear();
    h.run();
    h.state_mut().nodes = vec![one];
    h.run();
    assert_eq!(*made.borrow(), 2);
}
