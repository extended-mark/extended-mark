// Written by xtmd-schemagen from schemas/ir.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An object with nothing in it: `{}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Both ends are included; a missing one is open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Expr>,
}

/// A call of a fn by its full name: one written in Extended Mark, an extern, or an interface's, which goes to the fn of the value's own type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Expr>,
    pub name: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<Type>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseDecl {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    pub name: String,
}

/// Opening an `opt`: `if (let …)` or `if (mut …)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CondOpen {
    #[serde(rename = "mut", default, skip_serializing_if = "is_false")]
    pub mutable: bool,
    pub name: String,
    pub opt: Expr,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Cond {
    #[serde(rename = "cond")]
    Cond(Expr),
    #[serde(rename = "open")]
    Open(CondOpen),
}

/// What narrows a type. Every part that is given has to hold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclType {
    pub is: Type,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<TypeParam>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Decl {
    #[serde(rename = "record")]
    Record(Record),
    #[serde(rename = "enum")]
    Enum(Enum),
    #[serde(rename = "token")]
    Token(Token),
    #[serde(rename = "interface")]
    Interface(Interface),
    #[serde(rename = "fn")]
    Fn(FnDecl),
    #[serde(rename = "extern")]
    Extern(Extern),
    /// The document's one `$state` block.
    #[serde(rename = "$state")]
    State(State),
    #[serde(rename = "expose")]
    Expose(Expose),
    /// A top-level `let`: a constant, or a value worked out again when what it reads changes.
    #[serde(rename = "let")]
    Let(LetDecl),
    /// A type alias: another name for a type. A contract written after the alias's name adds to its own (`core/int` is `core/number` with step 1).
    #[serde(rename = "type")]
    Type(DeclType),
}

/// A declaration the page uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub declare: Decl,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enum {
    pub cases: Vec<CaseDecl>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    /// Its cases carry nothing, so they are ordered as written.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ordered: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<TypeParam>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposeFns {
    pub body: Vec<Stmt>,
    pub name: String,
    pub params: Vec<Param>,
    /// What it may write, the fns it calls included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<Root>,
}

/// Fns the host may call, answering the extern channel of the same name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expose {
    pub channel: String,
    pub fns: Vec<ExposeFns>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprRecord {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Expr>,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprCase {
    #[serde(rename = "enum")]
    pub enum_: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Expr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprTokval {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprTemplate {
    pub holes: Vec<Expr>,
    pub pieces: Vec<String>,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprField {
    pub name: String,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprApply {
    pub args: Vec<Expr>,
    #[serde(rename = "fn")]
    pub fn_: Expr,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<Type>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprOp {
    pub args: Vec<Expr>,
    pub name: OpName,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<Type>,
    /// Proved: what it works on, and the number it gives when it gives one, are whole and within 9007199254740991 of zero, so any engine's whole numbers hold them exactly.
    #[serde(default, skip_serializing_if = "is_false")]
    pub whole: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprShow {
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprTry {
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExprWith {
    pub fields: BTreeMap<String, Expr>,
    pub value: Expr,
}

/// An expression. Values are expressions too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    #[serde(rename = "num")]
    Num(String),
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "bool")]
    Bool(bool),
    #[serde(rename = "duration")]
    Duration(String),
    #[serde(rename = "list")]
    List(Vec<Expr>),
    /// A record made; its rules run when it is made.
    #[serde(rename = "record")]
    Record(ExprRecord),
    /// An enum's or a token's case, with its values.
    #[serde(rename = "case")]
    Case(ExprCase),
    #[serde(rename = "some")]
    Some(Box<Expr>),
    #[serde(rename = "none")]
    None(Empty),
    #[serde(rename = "range")]
    Range(Box<ExprRange>),
    /// A token, by its name. An engine draws it with its own value.
    #[serde(rename = "token")]
    Token(String),
    /// A token made from a plain value, through its case that carries one: `stroke.PX(2.5)`.
    #[serde(rename = "tokval")]
    Tokval(Box<ExprTokval>),
    /// A `core/rich_text`, its Markdown already read.
    #[serde(rename = "rich")]
    Rich(Rich),
    /// Whether a rich_text is a sentence, and not blocks.
    #[serde(rename = "sentence")]
    Sentence(Box<Expr>),
    /// A literal with holes: a text when `type` is `core/text`, a `core/template` otherwise.
    #[serde(rename = "template")]
    Template(Box<ExprTemplate>),
    /// A view as a value: a description of a call, drawn where it is placed.
    #[serde(rename = "view")]
    View(ViewValue),
    #[serde(rename = "get")]
    Get(Get),
    #[serde(rename = "field")]
    Field(Box<ExprField>),
    #[serde(rename = "call")]
    Call(Box<Call>),
    /// A call of a fn held in a value.
    #[serde(rename = "apply")]
    Apply(Box<ExprApply>),
    #[serde(rename = "op")]
    Op(Box<ExprOp>),
    /// A value shown as text, as a hole shows it.
    #[serde(rename = "show")]
    Show(Box<ExprShow>),
    #[serde(rename = "fn")]
    Fn(Box<Lambda>),
    /// Where a `mut` argument writes.
    #[serde(rename = "place")]
    Place(Place),
    /// The value, or `none` when one of its contracts or rules fails.
    #[serde(rename = "try")]
    Try(Box<ExprTry>),
    #[serde(rename = "with")]
    With(Box<ExprWith>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternFn {
    /// The default body, run when the supplier gives none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default: Vec<Stmt>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Type>,
    pub supplier: Supplier,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternChannelFns {
    pub name: String,
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Type>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternChannel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fns: Vec<ExternChannelFns>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    pub supplier: Supplier,
    /// The tokens the page itself is drawn with, from the block's `tokens` line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternType {
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<TypeParam>,
    pub supplier: Supplier,
}

/// Something supplied from outside Extended Mark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Extern {
    /// An extern fn: native to every engine (core's), a pack's, or the host's.
    #[serde(rename = "fn")]
    Fn(ExternFn),
    #[serde(rename = "widget")]
    Widget(ExternWidget),
    /// An extern block: values the host gives and fns Extended Mark calls.
    #[serde(rename = "channel")]
    Channel(ExternChannel),
    #[serde(rename = "type")]
    Type(ExternType),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternWidget {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<TypeParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Field>,
    /// The levels drawing this widget needs beyond core.draw: those its `uses` names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<Level>,
    /// What it writes as Markdown, from its markdown block. Left out when it writes nothing, or when it is a piece of Markdown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markdown: Vec<Node>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    /// The piece of Markdown it is, from `markdown #quote;`: `quote`. Its Markdown is that piece's, and `markdown` is left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece: Option<String>,
    /// What it expects of its inputs and slots: its `require` and `warn` blocks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Require>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
    pub supplier: Supplier,
    /// The tokens its look comes from: a case by its full name (`core/color.LINE`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<String>,
}

/// A record's field, a widget's input or own variable, or a `$state` field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Expr>,
    #[serde(rename = "mut", default, skip_serializing_if = "is_false")]
    pub mutable: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

/// An IR file: one build.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub ir: Module,
}

/// A fn written in Extended Mark. One with no return type is an action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FnDecl {
    pub body: Vec<Stmt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<TypeParam>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    /// What it reads beside its parameters, the fns it calls included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Type>,
    /// What it may write beside its `mut` parameters, the fns it calls included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<Root>,
}

/// A read of something by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Get {
    /// A local, a parameter or a loop variable.
    #[serde(rename = "local")]
    Local(String),
    /// An input of the widget the code is in.
    #[serde(rename = "input")]
    Input(String),
    /// A widget's own variable.
    #[serde(rename = "cell")]
    Cell(String),
    /// A field of the document's `$state`.
    #[serde(rename = "$state")]
    State(String),
    /// A top-level `let`, by its full name.
    #[serde(rename = "let")]
    Let(String),
    /// A named token: `core/space.S`.
    #[serde(rename = "token")]
    Token(String),
    /// A slot of the widget the code is in.
    #[serde(rename = "slot")]
    Slot(String),
    /// A value of an extern channel: `core/page.width`, `doc/scores.unit`.
    #[serde(rename = "extern")]
    Extern(String),
}

/// What runs when an event fires.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handler {
    pub body: Vec<Stmt>,
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    /// What it reads, the fns it calls included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    /// What it may write, the fns it calls included. An event it invokes writes what that event's handler does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<Root>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// The names of its fns. A call of one is sent to the fn of the value's own type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fns: Vec<String>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<TypeParam>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub widget: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lambda {
    pub body: Vec<Stmt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Type>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LetDecl {
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub value: Expr,
}

/// A level an engine has (page 6): core.draw draws the extern widgets, core.live holds variables and runs events, core.gfx draws the canvas widgets. An engine lists the ones it has, and a call, a widget or a page lists the ones it needs: none means its Markdown is enough.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Level {
    #[serde(rename = "core.draw")]
    CoreDraw,
    #[serde(rename = "core.live")]
    CoreLive,
    #[serde(rename = "core.gfx")]
    CoreGfx,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Local {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub value: Expr,
}

/// One build: the document (or pack) with the parts of core and of the given packs it uses, checked and lowered, ready to run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub declarations: Vec<Declaration>,
    /// A document's YAML front matter, without its --- lines, passed to the host untouched (page 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handlers: Vec<Handler>,
    /// Every level the page needs: the union of its calls'.
    pub levels: Vec<Level>,
    /// `doc` for a document, or the pack's name.
    pub module: String,
    pub needs: Needs,
    pub templates: Vec<Template>,
    /// The IR's version: a whole number of its own, apart from xtmd's version. 0.4 writes 4.
    pub version: i64,
    /// The version of the compiler that wrote it.
    pub xtmd: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsPacks {
    pub externs: Vec<String>,
    pub name: String,
    pub version: i64,
}

/// Every extern the page uses, by who supplies it. An engine checks the list when it mounts the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Needs {
    /// core's externs, supplied by the engine: widgets and fns.
    pub core: Vec<String>,
    /// Every value and fn taken from the host, by its full name: a value of core's page (`core/page.width`), and the values and fns of the document's own channels (`doc/scores.unit`).
    pub host: Vec<String>,
    pub packs: Vec<NeedsPacks>,
}

/// An extern widget, drawn by the engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeExtern {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub events: BTreeMap<String, String>,
    /// The levels this call needs beyond core.draw (page 6), its slots' included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<Level>,
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, Expr>,
    /// What each input reads, by its name. One that isn't here reads nothing that changes: it is the same for as long as the page is open. What is inside a view or a rich_text it holds says its own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reads: BTreeMap<String, Vec<Read>>,
    pub site: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Vec<Node>>,
    /// Where each `mut` input writes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub writes: BTreeMap<String, Place>,
}

/// A widget written in Extended Mark.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeUse {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Expr>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub events: BTreeMap<String, String>,
    /// The levels this call needs beyond core.draw (page 6), its slots' included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<Level>,
    pub name: String,
    /// What each argument reads, by its name, as an extern's inputs do.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reads: BTreeMap<String, Vec<Read>>,
    pub site: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Vec<Node>>,
}

/// Rows of a list follow their items; rows of a range are keyed by their number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeForKey {
    #[serde(rename = "item")]
    Item,
    #[serde(rename = "number")]
    Number,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeFor {
    pub body: Vec<Node>,
    /// Rows of a list follow their items; rows of a range are keyed by their number.
    pub key: NodeForKey,
    #[serde(rename = "mut", default, skip_serializing_if = "is_false")]
    pub mutable: bool,
    pub name: String,
    pub over: Expr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// What the list or the range reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    pub site: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIfBranches {
    /// What the condition reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    pub then: Vec<Node>,
    pub when: Cond,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIf {
    pub branches: Vec<NodeIfBranches>,
    #[serde(rename = "else", default, skip_serializing_if = "Vec::is_empty")]
    pub else_: Vec<Node>,
    pub site: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMatchCases {
    pub then: Vec<Node>,
    pub when: Vec<Pattern>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMatch {
    pub cases: Vec<NodeMatchCases>,
    #[serde(rename = "else", default, skip_serializing_if = "Vec::is_empty")]
    pub else_: Vec<Node>,
    /// What the value reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    pub site: String,
    pub value: Expr,
}

/// A view value or a slot, drawn here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodePlace {
    /// What the view reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    /// It goes in a sentence, so a rich_text placed here is one: blocks are a failure.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sentence: bool,
    pub site: String,
    pub view: Expr,
}

/// A `let` in a view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeDeclare {
    #[serde(rename = "let")]
    pub let_: Local,
    /// What the value reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
}

/// A node of a template's view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Node {
    #[serde(rename = "extern")]
    Extern(NodeExtern),
    #[serde(rename = "use")]
    Use(NodeUse),
    #[serde(rename = "for")]
    For(NodeFor),
    #[serde(rename = "if")]
    If(NodeIf),
    #[serde(rename = "match")]
    Match(NodeMatch),
    #[serde(rename = "place")]
    Place(NodePlace),
    #[serde(rename = "declare")]
    Declare(NodeDeclare),
}

/// An operator, by its symbol. `-` with one argument negates; `?` takes a condition and two values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OpName {
    #[serde(rename = "+")]
    Plus,
    #[serde(rename = "-")]
    Minus,
    #[serde(rename = "*")]
    Times,
    #[serde(rename = "/")]
    Divide,
    #[serde(rename = "%")]
    Rem,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "&&")]
    And,
    #[serde(rename = "||")]
    Or,
    #[serde(rename = "!")]
    Not,
    #[serde(rename = "?")]
    Pick,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    #[serde(rename = "mut", default, skip_serializing_if = "is_false")]
    pub mutable: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternCase {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binds: Vec<String>,
    #[serde(rename = "enum")]
    pub enum_: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    #[serde(rename = "case")]
    Case(PatternCase),
    #[serde(rename = "num")]
    Num(String),
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "bool")]
    Bool(bool),
}

/// Where a write goes: a root and a path of fields and items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Step>,
    pub root: Root,
}

/// Something that may be another value later, which a place in the page reads. Constants, tokens and a fn's own locals are none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Read {
    /// A local of the view around it: a loop's variable, a `let`, what a condition or a match binds.
    #[serde(rename = "local")]
    Local(String),
    /// An input of the widget the code is in.
    #[serde(rename = "input")]
    Input(String),
    /// A widget's own variable.
    #[serde(rename = "cell")]
    Cell(String),
    /// A field of the document's `$state`.
    #[serde(rename = "$state")]
    State(String),
    /// A slot of the widget the code is in.
    #[serde(rename = "slot")]
    Slot(String),
    /// A value of an extern channel: `core/page.width`.
    #[serde(rename = "extern")]
    Extern(String),
    /// It can't be known: a fn held in a variable is called. It is worked out again after every change.
    #[serde(rename = "any")]
    Any(Empty),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub fields: Vec<Field>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    /// Type parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<TypeParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Require>,
    /// The fn `==` and `!=` call, when the record's pack writes one. Left out, records compare field by field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same: Option<String>,
}

/// A rule, checked whenever a value it reads changes: `require` refuses, `warn` only tells the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Require {
    pub body: Vec<Stmt>,
    pub message: String,
    /// What the rule reads, the fns it calls included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<Read>,
    /// A `warn` block: false is told, and refuses nothing.
    #[serde(default, skip_serializing_if = "is_false")]
    pub warn: bool,
}

/// A rich_text: calls, as a value. A sentence's are drawn among words; `blocks` says they are blocks instead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rich {
    #[serde(default, skip_serializing_if = "is_false")]
    pub blocks: bool,
    pub nodes: std::sync::Arc<Vec<Node>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Root {
    #[serde(rename = "local")]
    Local(String),
    #[serde(rename = "input")]
    Input(String),
    #[serde(rename = "cell")]
    Cell(String),
    #[serde(rename = "$state")]
    State(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

/// What a document's widgets share.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub fields: Vec<Field>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Require>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepField {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepItem {
    pub at: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Step {
    #[serde(rename = "field")]
    Field(StepField),
    #[serde(rename = "item")]
    Item(StepItem),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StmtDeclare {
    #[serde(rename = "let")]
    Let(Local),
    #[serde(rename = "mut")]
    Mut(Local),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtSet {
    pub place: Place,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtReturn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtIfBranches {
    pub then: Vec<Stmt>,
    pub when: Cond,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtIf {
    pub branches: Vec<StmtIfBranches>,
    #[serde(rename = "else", default, skip_serializing_if = "Vec::is_empty")]
    pub else_: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtFor {
    pub body: Vec<Stmt>,
    #[serde(rename = "mut", default, skip_serializing_if = "is_false")]
    pub mutable: bool,
    pub name: String,
    pub over: Expr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtMatchCases {
    pub then: Vec<Stmt>,
    pub when: Vec<Pattern>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtMatch {
    pub cases: Vec<StmtMatchCases>,
    #[serde(rename = "else", default, skip_serializing_if = "Vec::is_empty")]
    pub else_: Vec<Stmt>,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtInvoke {
    pub args: Vec<Expr>,
    pub event: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StmtApply {
    pub args: Vec<Expr>,
    #[serde(rename = "fn")]
    pub fn_: Expr,
}

/// A statement of a fn, a handler or a rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// A local.
    #[serde(rename = "declare")]
    Declare(StmtDeclare),
    #[serde(rename = "set")]
    Set(StmtSet),
    #[serde(rename = "return")]
    Return(StmtReturn),
    #[serde(rename = "if")]
    If(StmtIf),
    /// `over` is a list, a range, or for `mut for` a place.
    #[serde(rename = "for")]
    For(StmtFor),
    #[serde(rename = "match")]
    Match(StmtMatch),
    #[serde(rename = "invoke")]
    Invoke(StmtInvoke),
    /// An action.
    #[serde(rename = "call")]
    Call(Call),
    /// An action held in a value.
    #[serde(rename = "apply")]
    Apply(StmtApply),
}

/// Who supplies an extern: the engine (core's), the pack that declares it, or the host (a document's channels).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Supplier {
    #[serde(rename = "core")]
    Core,
    #[serde(rename = "pack")]
    Pack,
    #[serde(rename = "host")]
    Host,
}

/// A widget written in Extended Mark, lowered: its view as nodes. The document's own view is the template `doc`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// Its own variables, each with its starting value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Field>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fns: Vec<FnDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generics: Vec<TypeParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Field>,
    /// The levels this widget needs wherever it is called, beyond core.draw: its own variables need core.live, and it needs every level its view's calls need.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<Level>,
    /// Its markdown block, lowered onto core's extern Markdown blocks; built from the view for a document's widget that has none. Left out when it is the view itself, and empty when it writes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<Vec<Node>>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Require>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
    pub view: Vec<Node>,
}

/// A token type: an enum whose cases stand for values of one type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    pub cases: Vec<CaseDecl>,
    /// A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`).
    pub name: String,
    /// The type of the value each case stands for.
    pub value: Type,
    /// Each named case's value, from the tokens blocks of the build. An engine may give its own.
    pub values: BTreeMap<String, Expr>,
}

/// A type. `name` is a built-in type (`core/number`, `core/list`, …), a declared one (an alias among them), a type parameter (capitals), or `fn`, `record` or `any`, which use the parts that follow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Type {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Type>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    /// For `record`: an inline record's fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// For `any`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
    pub name: String,
    /// For `fn`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    /// For `fn`; left out when it returns nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Box<Type>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeParam {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounds: Vec<String>,
    /// Capitals: `T`.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValue {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Expr>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub events: BTreeMap<String, String>,
    /// What each argument reads, by its name, as an extern's inputs do.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reads: BTreeMap<String, Vec<Read>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Vec<Node>>,
    pub widget: String,
}
