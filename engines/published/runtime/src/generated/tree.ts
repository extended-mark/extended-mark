// Written by xtmd-schemagen from schemas/tree.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

/** What changed in one node. What isn't here is as it was. A widget that failed, or draws again, is another node to its parent. */
export interface Change {
  /** The inputs whose widgets are others now, or in another order. */
  children?: { [key: string]: Child[] };
  /** The events now, when they changed. */
  events?: string[];
  /** The node, or the page itself, whose id is empty and whose one input is `nodes`. */
  id: string;
  /** The inputs that changed, whole. */
  inputs?: { [key: string]: Input };
  /** The `mut` inputs now, when what a write has to fit changed. */
  writes?: { [key: string]: Contract };
}

export type Child =
  | { keep: string }
  | { node: Node };

/** What a value has to fit. Every part that is given has to hold. */
export interface Contract {
  length?: Ends;
  only?: Input[];
  range?: Ends;
  step?: Input;
}

/** The page's data: what a host keeps to put a page back as it was. Plain values only. */
export interface Data {
  /** Each widget's own variables, by the place it is drawn at. */
  own?: { [key: string]: { [key: string]: Input } };
  /** `$state`, by field. */
  state: { [key: string]: Input };
}

/** Both ends are included; a missing one is open. */
export interface Ends {
  high?: Input;
  low?: Input;
}

export interface InputRecord {
  fields: { [key: string]: Input };
  type: string;
}

export interface InputCase {
  enum: string;
  name: string;
  values?: { [key: string]: Input };
}

export interface InputRange {
  high?: Input;
  low?: Input;
  step?: Input;
}

export interface InputTokval {
  name: string;
  type: string;
  value: Input;
}

export interface InputFn {
  /** Moves when the fn may answer otherwise: what it was given, or what it read, changed. */
  version: number;
}

/** An input's value, as the IR writes one; and what only a drawn page holds: widgets, and a fn to call. */
export type Input =
  | { num: string }
  | { text: string }
  | { bool: boolean }
  | { duration: string }
  | { list: Input[] }
  | { record: InputRecord }
  | { case: InputCase }
  | { some: Input }
  | { none: { [key: string]: never } }
  | { range: InputRange }
  | { token: string }
  | { tokval: InputTokval }
  | { nodes: Node[] }
  | { fn: InputFn };

/** A widget to draw. */
export interface Node {
  /** The events the page does something with. */
  events?: string[];
  /** It couldn't be drawn, and why. Its inputs are empty, and it is drawn again once it can be. */
  failed?: string;
  /** Who it is: its own on the page, and the same for as long as it is there. A loop's widget follows its item, so a row moved keeps its id. */
  id: string;
  /** Every input and slot by its name, worked out, each with its default when the call gave none. A slot's is the widgets in it. */
  inputs: { [key: string]: Input };
  /** The extern widget it calls, by its full name: `core/press`. Of a widget that failed, the one that did. */
  widget: string;
  /** The `mut` inputs, which the host may write, with what a write has to fit: the variable's the call gave, or the input's own. */
  writes?: { [key: string]: Contract };
}

/** The page now: its extern widgets in order, each with everything in it. */
export interface Tree {
  nodes: Node[];
}
