// Nodes of a drawn tree as elements: each by its widget, keyed by its id across changes (a loop row's widgets
// follow its item) and marked with it, so focus and what is being typed survive a redraw whatever renders it.

import type { Input, Node as Drawn } from "@extended-mark/runtime";
import { nodes } from "./values.js";
import type { Tokens } from "./tokens.js";
import { h, type Child, type VNode } from "./vnode.js";

/** What a widget can do where it is drawn. */
export interface ViewContext {
  readonly tokens: Tokens;
  /** A prefix unique to this document, for element ids (`xtmdv3-`). */
  readonly uid: string;
  /** The node as it is now: the latest render may not be on the page yet. */
  node(): Drawn;
  /** The reader's event on this widget: runs the caller's handler, as one change. */
  event(name: string, args?: Input[]): void;
  /** This widget writing one of its `mut` inputs, as a field or a slider does: checked, then one change. */
  write(input: string, value: Input): void;
  /** A fn input called, as a shade calls its shader. */
  call(input: string, args: Input[]): Input;
  /** An input's widgets, drawn. */
  children(nodes: readonly Drawn[]): Child[];
  /** Asks for the layout, once for each change from outside it, when this render is in the document; `again`
   * asks whatever was asked before, as when an element's own size changed. */
  layout(again?: boolean): void;
  /** Tells what went wrong in the widget's own code. */
  fail(f: unknown): void;
}

/** A widget: its element, or words, for what the page shows now. Core's are in `widgets`; a pack's own extern
 * widgets are given the same way, by full name. */
export type ViewWidget = (d: Drawn, ctx: ViewContext) => Child;

function notice(message: string): VNode {
  return h("div", "notice", { role: "alert" }, [message]);
}

/** What draws a page's nodes, and what their widgets do. */
export interface Drawing {
  readonly registry: Record<string, ViewWidget>;
  /** The host's own components, by the widget each draws. */
  readonly components?: Record<string, unknown>;
  /** What is drawn where a widget failed. */
  readonly failed?: ViewWidget;
  readonly tokens: Tokens;
  readonly uid: string;
  node(id: string): Drawn | undefined;
  event(id: string, name: string, args: Input[]): void;
  write(id: string, input: string, value: Input): void;
  call(id: string, input: string, args: Input[]): Input;
  layout(id: string, again: boolean): void;
  fail(f: unknown): void;
  /** What nodes were drawn as: a node that is the same one draws the same. Left out, all are drawn anew. */
  readonly kept?: WeakMap<Drawn, Child>;
}

/** A node as its element or words, marked with its id and its widget. */
export function drawn(dr: Drawing, d: Drawn): Child {
  const was = dr.kept?.get(d);
  if (was !== undefined) return was;
  const v = draw(dr, d);
  if (typeof v !== "string") {
    v.key = d.id;
    v.attrs["data-xtmd-site"] = d.id;
    v.attrs["data-xtmd-widget"] = d.widget;
  }
  dr.kept?.set(d, v);
  return v;
}

/** Nodes as elements, each keyed once among its siblings. */
export function children(dr: Drawing, nodes: readonly Drawn[]): Child[] {
  const out: Child[] = [];
  for (const d of nodes) {
    const v = drawn(dr, d);
    if (typeof v !== "string") out.push(v);
    else if (typeof out[out.length - 1] === "string") out[out.length - 1] += v;
    // words that follow words are one text, as a document read back from HTML has them
    else if (v) out.push(v);
  }
  return out;
}

/** A widget drawn by the host's component: in an element of its own, which lays out nothing. */
function componentOf(dr: Drawing, d: Drawn, of: unknown): VNode {
  const v = h("xtmd-widget");
  v.style = { display: "contents" };
  v.component = {
    of,
    props: {
      node: d,
      event: (name, args = []) => dr.event(d.id, name, args),
      write: (input, value) => dr.write(d.id, input, value),
      call: (input, args) => dr.call(d.id, input, args),
    },
    children: (input) => children(dr, nodes(d.inputs[input])),
  };
  return v;
}

function draw(dr: Drawing, d: Drawn): Child {
  if (d.failed !== undefined && !dr.failed) return notice(`${d.widget.split("/").pop()} failed: ${d.failed}`);
  const of = d.failed === undefined ? dr.components?.[d.widget] : undefined;
  if (of !== undefined) return componentOf(dr, d, of);
  const maker = d.failed !== undefined ? dr.failed : dr.registry[d.widget];
  // an extern widget nobody supplies shows its Markdown instead (page 6): the page draws it with
  // the blocks we do draw, so this is only one whose supplier the page wasn't told about
  if (!maker) return notice(`Nothing draws ${d.widget} here: its widget isn't supplied.`);
  const ctx: ViewContext = {
    tokens: dr.tokens,
    uid: dr.uid,
    node: () => dr.node(d.id) ?? d,
    event: (name, args = []) => dr.event(d.id, name, args),
    write: (input, value) => dr.write(d.id, input, value),
    call: (input, args) => dr.call(d.id, input, args),
    children: (nodes) => children(dr, nodes),
    layout: (again = false) => dr.layout(d.id, again),
    fail: (f) => dr.fail(f),
  };
  try {
    return maker(d, ctx);
  } catch (f) {
    dr.fail(f);
    return notice(`${d.widget.split("/").pop()} couldn't be drawn: ${f instanceof Error ? f.message : String(f)}`);
  }
}
