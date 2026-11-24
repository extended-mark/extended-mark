//! What is the same on every page is worked out now: an expression made of nothing that can change is
//! its value. Only plain values (a number, a text, a bool, a duration): the rest is as short as it was.

use std::collections::HashSet;

use xtmd_eval::interp::{FailKind, Failure, Host, Machine, Outer, Program};
use xtmd_eval::value::Case;
use xtmd_eval::Value;
use xtmd_ir::ir::{self, Decl, Expr, Get};

use crate::walk;

/// Constants alone. A token's value is the engine's, so it is none.
struct Closed;

impl Host for Closed {
    fn read(&self, m: &mut Machine, what: &Outer, _owner: Option<u64>) -> Result<Value, Failure> {
        match what {
            Outer::Let(n) if m.prog.lets.contains_key(&**n) => Ok(m.prog.lets[&**n].clone()),
            _ => Err(unknown()),
        }
    }

    fn write(&self, _m: &mut Machine, _what: &Outer, _owner: Option<u64>, _value: Value) -> Result<(), Failure> {
        Err(unknown())
    }

    fn token_value(&self, _m: &mut Machine, _case: &Case) -> Result<Value, Failure> {
        Err(unknown())
    }
}

fn unknown() -> Failure {
    Failure::new(FailKind::NotConstant, "only known when the page runs".to_string())
}

/// Whether anything was worked out, so that less of the module may be needed.
pub fn fold(m: &mut ir::Module) -> bool {
    let mut prog = Program::default();
    for d in &m.declarations {
        prog.add(&d.declare);
    }
    // each let, once the ones it reads are known
    let mut left: Vec<&ir::LetDecl> = m.declarations.iter().filter_map(|d| if let Decl::Let(l) = &d.declare { Some(l) } else { None }).collect();
    loop {
        let before = left.len();
        left.retain(|l| {
            let v = Machine::new(&prog, Box::new(Closed)).eval(&l.value);
            match v {
                Ok(v) => {
                    prog.lets.insert(l.name.clone(), v);
                    false
                }
                Err(_) => true,
            }
        });
        if left.len() == before || left.is_empty() {
            break;
        }
    }
    let lets: HashSet<String> = prog.lets.keys().cloned().collect();
    let mut folded = false;
    // a record written whole had its rules run when it was checked: one that breaks them is left as it is,
    // to be made, and to fail, as the page runs
    let mut left: HashSet<*const Expr> = HashSet::new();
    walk::module(m, &mut |p| {
        let walk::Part::Expr(e) = p else { return };
        if let Expr::Record(_) = e {
            if closed(e, &lets) && Machine::new(&prog, Box::new(Closed)).eval(e).is_err() {
                walk::expr(e, &mut |p| {
                    if let walk::Part::Expr(x) = p {
                        left.insert(x as *const Expr);
                    }
                });
            }
        }
        if left.contains(&(e as *const Expr)) || !works(e) || !closed(e, &lets) {
            return;
        }
        let mut machine = Machine::new(&prog, Box::new(Closed));
        let out = machine.eval(e);
        // a `warn` found false is told each time, as the page runs
        if !machine.warned.is_empty() {
            return;
        }
        if let Ok(v @ (Value::Num(_) | Value::Text(_) | Value::Bool(_) | Value::Duration(_))) = out {
            *e = v.to_ir();
            folded = true;
        }
    });
    folded
}

/// Whether it is worked out at all, and isn't a value already.
fn works(e: &Expr) -> bool {
    matches!(e, Expr::Op(_) | Expr::Call(_) | Expr::Show(_) | Expr::Field(_) | Expr::Template(_) | Expr::Get(Get::Let(_)))
}

/// Made of nothing that can change.
fn closed(e: &Expr, lets: &HashSet<String>) -> bool {
    let all = |xs: &[Expr]| xs.iter().all(|x| closed(x, lets));
    match e {
        Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) | Expr::Token(_) => true,
        Expr::Get(Get::Let(n)) => lets.contains(n),
        Expr::List(xs) => all(xs),
        Expr::Record(r) => r.fields.values().all(|x| closed(x, lets)),
        Expr::Case(c) => c.values.values().all(|x| closed(x, lets)),
        Expr::Some(x) => closed(x, lets),
        Expr::Range(r) => [&r.low, &r.high, &r.step].into_iter().flatten().all(|x| closed(x, lets)),
        Expr::Tokval(t) => closed(&t.value, lets),
        Expr::Template(t) => all(&t.holes),
        Expr::Field(f) => closed(&f.value, lets),
        Expr::Call(c) => all(&c.args),
        Expr::Op(o) => all(&o.args),
        Expr::Show(s) => closed(&s.value, lets),
        Expr::Try(t) => closed(&t.value, lets),
        Expr::With(w) => closed(&w.value, lets) && w.fields.values().all(|x| closed(x, lets)),
        _ => false,
    }
}
