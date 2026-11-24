// Written by xtmd-schemagen from schemas/web.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

/** The format's version. */
export const VERSION = 1;

/** What a widget draws. */
export type Drawn =
  | { words: string }
  | { element: Element };

/** One element. */
export interface Element {
  /** Its attributes, as HTML writes them. */
  attrs?: { [key: string]: string };
  children?: Drawn[];
  /** The widget a host's own component draws in it, by full name. */
  component?: string;
  /** Only the live element can do the rest: a canvas painted, a plane measured. The widget's code is told as it enters the page, after each change, and as it leaves. */
  hook?: boolean;
  /** Who it is among the ones beside it, across redraws: a widget's is its node's id. */
  key?: string;
  /** The DOM events it listens for. What it does then is the widget's code. */
  listens?: string[];
  /** What the live element holds that no attribute does: a field's `value`, a box's `checked`. */
  props?: { [key: string]: Prop };
  /** CSS properties, in CSS's spelling. */
  style?: { [key: string]: string };
  /** It is in SVG's namespace. */
  svg?: boolean;
  tag: string;
}

export type Prop =
  | { text: string }
  | { bool: boolean };
