// Written by xtmd-schemagen from schemas/ir.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

/** Both ends are included; a missing one is open. */
export interface Bounds {
  high?: Expr;
  low?: Expr;
}

/** A call of a fn by its full name: one written in Extended Mark, an extern, or an interface's, which goes to the fn of the value's own type. */
export interface Call {
  args?: Expr[];
  name: string;
  type?: Type;
}

export interface CaseDecl {
  fields?: Field[];
  name: string;
}

/** Opening an `opt`: `if (let …)` or `if (mut …)`. */
export interface CondOpen {
  mut?: boolean;
  name: string;
  opt: Expr;
  type: Type;
}

export type Cond =
  | { cond: Expr }
  | { open: CondOpen };

/** What narrows a type. Every part that is given has to hold. */
export interface Contract {
  length?: Bounds;
  only?: Expr[];
  range?: Bounds;
  step?: Expr;
}

export interface DeclType {
  is: Type;
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  params?: TypeParam[];
}

export type Decl =
  | { record: Record }
  | { enum: Enum }
  | { token: Token }
  | { interface: Interface }
  | { fn: FnDecl }
  | { extern: Extern }
  | { "$state": State }
  | { expose: Expose }
  | { let: LetDecl }
  | { type: DeclType };

/** A declaration the page uses. */
export interface Declaration {
  declare: Decl;
}

export interface Enum {
  cases: CaseDecl[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  /** Its cases carry nothing, so they are ordered as written. */
  ordered?: boolean;
  params?: TypeParam[];
}

export interface Event {
  name: string;
  params?: Param[];
}

export interface ExposeFns {
  body: Stmt[];
  name: string;
  params: Param[];
}

/** Fns the host may call, answering the extern channel of the same name. */
export interface Expose {
  channel: string;
  fns: ExposeFns[];
}

export interface ExprRecord {
  fields?: { [key: string]: Expr };
  type: string;
}

export interface ExprCase {
  enum: string;
  name: string;
  values?: { [key: string]: Expr };
}

export interface ExprRange {
  high?: Expr;
  low?: Expr;
  step?: Expr;
}

export interface ExprTokval {
  name: string;
  type: string;
  value: Expr;
}

export interface ExprTemplate {
  holes: Expr[];
  pieces: string[];
  type: Type;
}

export interface ExprField {
  name: string;
  value: Expr;
}

export interface ExprApply {
  args: Expr[];
  fn: Expr;
  type?: Type;
}

export interface ExprOp {
  args: Expr[];
  name: OpName;
  type?: Type;
}

export interface ExprShow {
  value: Expr;
}

export interface ExprTry {
  value: Expr;
}

export interface ExprWith {
  fields: { [key: string]: Expr };
  value: Expr;
}

/** An expression. Values are expressions too. */
export type Expr =
  | { num: string }
  | { text: string }
  | { bool: boolean }
  | { duration: string }
  | { list: Expr[] }
  | { record: ExprRecord }
  | { case: ExprCase }
  | { some: Expr }
  | { none: { [key: string]: never } }
  | { range: ExprRange }
  | { token: string }
  | { tokval: ExprTokval }
  | { rich: Rich }
  | { sentence: Expr }
  | { template: ExprTemplate }
  | { view: ViewValue }
  | { get: Get }
  | { field: ExprField }
  | { call: Call }
  | { apply: ExprApply }
  | { op: ExprOp }
  | { show: ExprShow }
  | { fn: Lambda }
  | { place: Place }
  | { try: ExprTry }
  | { with: ExprWith };

export interface ExternFn {
  /** The default body, run when the supplier gives none. */
  default?: Stmt[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  params?: Param[];
  returns?: Type;
  supplier: Supplier;
}

export interface ExternChannelFns {
  name: string;
  params: Param[];
  returns?: Type;
}

export interface ExternChannel {
  fns?: ExternChannelFns[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  supplier: Supplier;
  /** The tokens the page itself is drawn with, from the block's `tokens` line. */
  tokens?: string[];
  values?: Field[];
}

export interface ExternType {
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  params?: TypeParam[];
  supplier: Supplier;
}

/** Something supplied from outside Extended Mark. */
export type Extern =
  | { fn: ExternFn }
  | { widget: ExternWidget }
  | { channel: ExternChannel }
  | { type: ExternType };

export interface ExternWidget {
  events?: Event[];
  generics?: TypeParam[];
  inputs?: Field[];
  /** The levels drawing this widget needs beyond core.draw: those its `uses` names. */
  levels?: Level[];
  /** What it writes as Markdown, from its markdown block. Left out when it writes nothing, or when it is a piece of Markdown. */
  markdown?: Node[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  /** The piece of Markdown it is, from `markdown #quote;`: `quote`. Its Markdown is that piece's, and `markdown` is left out. */
  piece?: string;
  /** What it expects of its inputs and slots: its `require` and `warn` blocks. */
  requires?: Require[];
  slots?: Slot[];
  supplier: Supplier;
  /** The tokens its look comes from: a case by its full name (`core/color.LINE`). */
  tokens?: string[];
}

/** A record's field, a widget's input or own variable, or a `$state` field. */
export interface Field {
  default?: Expr;
  mut?: boolean;
  name: string;
  type: Type;
}

/** An IR file: one build. */
export interface File {
  ir: Module;
}

/** A fn written in Extended Mark. One with no return type is an action. */
export interface FnDecl {
  body: Stmt[];
  generics?: TypeParam[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  params?: Param[];
  returns?: Type;
}

/** A read of something by name. */
export type Get =
  | { local: string }
  | { input: string }
  | { cell: string }
  | { "$state": string }
  | { let: string }
  | { token: string }
  | { slot: string }
  | { extern: string };

/** What runs when an event fires. */
export interface Handler {
  body: Stmt[];
  id: string;
  params?: Param[];
}

export interface Interface {
  events?: Event[];
  fields?: Field[];
  /** The names of its fns. A call of one is sent to the fn of the value's own type. */
  fns?: string[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  params?: TypeParam[];
  widget?: boolean;
}

export interface Lambda {
  body: Stmt[];
  params?: Param[];
  returns?: Type;
}

export interface LetDecl {
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  type: Type;
  value: Expr;
}

/** A level an engine has (page 6): core.draw draws the extern widgets, core.live holds variables and runs events, core.gfx draws the canvas widgets. An engine lists the ones it has, and a call, a widget or a page lists the ones it needs: none means its Markdown is enough. */
export type Level = "core.draw" | "core.live" | "core.gfx";

export interface Local {
  name: string;
  type: Type;
  value: Expr;
}

/** One build: the document (or pack) with the parts of core and of the given packs it uses, checked and lowered, ready to run. */
export interface Module {
  declarations: Declaration[];
  /** A document's YAML front matter, without its --- lines, passed to the host untouched (page 1). */
  front_matter?: string;
  handlers?: Handler[];
  /** Every level the page needs: the union of its calls'. */
  levels: Level[];
  /** `doc` for a document, or the pack's name. */
  module: string;
  needs: Needs;
  templates: Template[];
  /** The IR's version: a whole number of its own, apart from xtmd's version. 0.4 writes 4. */
  version: number;
  /** The version of the compiler that wrote it. */
  xtmd: string;
}

export interface NeedsPacks {
  externs: string[];
  name: string;
  version: number;
}

/** Every extern the page uses, by who supplies it. An engine checks the list when it mounts the page. */
export interface Needs {
  /** core's externs, supplied by the engine: widgets and fns. */
  core: string[];
  /** Every value and fn taken from the host, by its full name: a value of core's page (`core/page.width`), and the values and fns of the document's own channels (`doc/scores.unit`). */
  host: string[];
  packs: NeedsPacks[];
}

/** An extern widget, drawn by the engine. */
export interface NodeExtern {
  events?: { [key: string]: string };
  /** The levels this call needs beyond core.draw (page 6), its slots' included. */
  levels?: Level[];
  name: string;
  props?: { [key: string]: Expr };
  site: string;
  slots?: { [key: string]: Node[] };
  /** Where each `mut` input writes. */
  writes?: { [key: string]: Place };
}

/** A widget written in Extended Mark. */
export interface NodeUse {
  args?: { [key: string]: Expr };
  events?: { [key: string]: string };
  /** The levels this call needs beyond core.draw (page 6), its slots' included. */
  levels?: Level[];
  name: string;
  site: string;
  slots?: { [key: string]: Node[] };
}

/** Rows of a list follow their items; rows of a range are keyed by their number. */
export type NodeForKey = "item" | "number";

export interface NodeFor {
  body: Node[];
  /** Rows of a list follow their items; rows of a range are keyed by their number. */
  key: NodeForKey;
  mut?: boolean;
  name: string;
  over: Expr;
  position?: string;
  site: string;
  type: Type;
}

export interface NodeIfBranches {
  then: Node[];
  when: Cond;
}

export interface NodeIf {
  branches: NodeIfBranches[];
  else?: Node[];
  site: string;
}

export interface NodeMatchCases {
  then: Node[];
  when: Pattern[];
}

export interface NodeMatch {
  cases: NodeMatchCases[];
  else?: Node[];
  site: string;
  value: Expr;
}

/** A view value or a slot, drawn here. */
export interface NodePlace {
  /** It goes in a sentence, so a rich_text placed here is one: blocks are a failure. */
  sentence?: boolean;
  site: string;
  view: Expr;
}

/** A `let` in a view. */
export interface NodeDeclare {
  let: Local;
}

/** A node of a template's view. */
export type Node =
  | { extern: NodeExtern }
  | { use: NodeUse }
  | { for: NodeFor }
  | { if: NodeIf }
  | { match: NodeMatch }
  | { place: NodePlace }
  | { declare: NodeDeclare };

/** An operator, by its symbol. `-` with one argument negates; `?` takes a condition and two values. */
export type OpName = "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "!" | "?";

export interface Param {
  mut?: boolean;
  name: string;
  type: Type;
}

export interface PatternCase {
  binds?: string[];
  enum: string;
  name: string;
}

export type Pattern =
  | { case: PatternCase }
  | { num: string }
  | { text: string }
  | { bool: boolean };

/** Where a write goes: a root and a path of fields and items. */
export interface Place {
  path?: Step[];
  root: Root;
}

export interface Record {
  fields: Field[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  /** Type parameters. */
  params?: TypeParam[];
  requires?: Require[];
  /** The fn `==` and `!=` call, when the record's pack writes one. Left out, records compare field by field. */
  same?: string;
}

/** A rule, checked whenever a value it reads changes: `require` refuses, `warn` only tells the host. */
export interface Require {
  body: Stmt[];
  message: string;
  /** A `warn` block: false is told, and refuses nothing. */
  warn?: boolean;
}

/** A rich_text: calls, as a value. A sentence's are drawn among words; `blocks` says they are blocks instead. */
export interface Rich {
  blocks?: boolean;
  nodes: Node[];
}

export type Root =
  | { local: string }
  | { input: string }
  | { cell: string }
  | { "$state": string };

export interface Slot {
  name: string;
  type: Type;
}

/** What a document's widgets share. */
export interface State {
  fields: Field[];
  requires?: Require[];
}

export interface StepField {
  name: string;
}

export interface StepItem {
  at: Expr;
}

export type Step =
  | { field: StepField }
  | { item: StepItem };

export type StmtDeclare =
  | { let: Local }
  | { mut: Local };

export interface StmtSet {
  place: Place;
  value: Expr;
}

export interface StmtReturn {
  value?: Expr;
}

export interface StmtIfBranches {
  then: Stmt[];
  when: Cond;
}

export interface StmtIf {
  branches: StmtIfBranches[];
  else?: Stmt[];
}

export interface StmtFor {
  body: Stmt[];
  mut?: boolean;
  name: string;
  over: Expr;
  position?: string;
  type: Type;
}

export interface StmtMatchCases {
  then: Stmt[];
  when: Pattern[];
}

export interface StmtMatch {
  cases: StmtMatchCases[];
  else?: Stmt[];
  value: Expr;
}

export interface StmtInvoke {
  args: Expr[];
  event: string;
}

export interface StmtApply {
  args: Expr[];
  fn: Expr;
}

/** A statement of a fn, a handler or a rule. */
export type Stmt =
  | { declare: StmtDeclare }
  | { set: StmtSet }
  | { return: StmtReturn }
  | { if: StmtIf }
  | { for: StmtFor }
  | { match: StmtMatch }
  | { invoke: StmtInvoke }
  | { call: Call }
  | { apply: StmtApply };

/** Who supplies an extern: the engine (core's), the pack that declares it, or the host (a document's channels). */
export type Supplier = "core" | "pack" | "host";

/** A widget written in Extended Mark, lowered: its view as nodes. The document's own view is the template `doc`. */
export interface Template {
  /** Its own variables, each with its starting value. */
  cells?: Field[];
  events?: Event[];
  fns?: FnDecl[];
  generics?: TypeParam[];
  inputs?: Field[];
  /** The levels this widget needs wherever it is called, beyond core.draw: its own variables need core.live, and it needs every level its view's calls need. */
  levels?: Level[];
  /** Its markdown block, lowered onto core's extern Markdown blocks; built from the view for a document's widget that has none. Left out when it is the view itself, and empty when it writes nothing. */
  markdown?: Node[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  requires?: Require[];
  slots?: Slot[];
  view: Node[];
}

/** A token type: an enum whose cases stand for values of one type. */
export interface Token {
  cases: CaseDecl[];
  /** A declaration's full name: its pack, a slash, its name (`core/date`, `doc/task`). A fn on a type adds a dot and the fn's name (`core/list.some`). */
  name: string;
  /** The type of the value each case stands for. */
  value: Type;
  /** Each named case's value, from the tokens blocks of the build. An engine may give its own. */
  values: { [key: string]: Expr };
}

/** A type. `name` is a built-in type (`core/number`, `core/list`, …), a declared one (an alias among them), a type parameter (capitals), or `fn`, `record` or `any`, which use the parts that follow. */
export interface Type {
  args?: Type[];
  contract?: Contract;
  /** For `record`: an inline record's fields. */
  fields?: Field[];
  /** For `any`. */
  interface?: string;
  name: string;
  /** For `fn`. */
  params?: Param[];
  /** For `fn`; left out when it returns nothing. */
  returns?: Type;
}

export interface TypeParam {
  bounds?: string[];
  /** Capitals: `T`. */
  name: string;
}

export interface ViewValue {
  args?: { [key: string]: Expr };
  events?: { [key: string]: string };
  slots?: { [key: string]: Node[] };
  widget: string;
}
