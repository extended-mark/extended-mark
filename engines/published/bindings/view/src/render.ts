// A page's tree as elements: each node by its widget, keyed by its id across changes (a loop row's
// widgets follow its item) and marked with it, as
// @extended-mark/dom marks its own, so focus and what is being typed survive a redraw in every framework.

import type { Input, Node as Drawn, Page } from "@extended-mark/runtime";
import { nodes, styleText, Tokens } from "@extended-mark/dom/shared";
import { h, type Child, type VNode } from "./vnode.js";

/** What a widget can do on the page it is drawn on. */
export interface ViewContext {
  readonly page: Page;
  readonly tokens: Tokens;
  /** A prefix unique to this document, for element ids (`xtmdv3-`). */
  readonly uid: string;
  /** The reader's event on this widget: runs the caller's handler, as one change. */
  event(name: string, args?: Input[]): void;
  /** This widget writing one of its `mut` inputs, as a field or a slider does: checked, then one change. */
  write(input: string, value: Input): void;
  /** A fn input called, as a shade calls its shader. */
  call(input: string, args: Input[]): Input;
  /** An input's widgets, drawn. */
  children(nodes: readonly Drawn[]): Child[];
  /** Asks for the plane's layout once this render is in the document. */
  layout(): void;
}

/** A widget: its element, or words, for what the page shows now. Core's are in `widgets`; a pack's own extern
 * widgets are given the same way, by full name. */
export type ViewWidget = (d: Drawn, ctx: ViewContext) => Child;

function notice(message: string): VNode {
  return h("div", "notice", { role: "alert" }, [message]);
}

export interface Drawing {
  readonly page: Page;
  readonly registry: Record<string, ViewWidget>;
  /** The host's own components, by the widget each draws. */
  readonly components?: Record<string, unknown>;
  /** What is drawn where a widget failed. */
  readonly failed?: ViewWidget;
  readonly tokens: Tokens;
  readonly uid: string;
  /** Called when a widget asks for the plane's layout. */
  layout(): void;
  /** Called after a reader's event or write, done or refused, so what it did reaches the host. */
  after?(): void;
}

/** The page as one root element: its stylesheet, its tokens for both themes, and every
 * drawn widget. */
export function renderPage(dr: Drawing): VNode {
  const sheet = dr.tokens.sheet();
  const root = h("div", undefined, { class: `xtmd-root ${sheet.name}`, "data-xtmd-root": "", "data-xtmd-theme": dr.tokens.theme().toLowerCase() }, [
    { tag: "style", attrs: { "data-xtmd-dom": "" }, children: [styleText + sheet.css] },
    ...children(dr, dr.page.tree().nodes),
  ]);
  return root;
}

/** Drawn widgets as elements, each keyed once among its siblings. */
export function children(dr: Drawing, nodes: readonly Drawn[]): Child[] {
  const out: Child[] = [];
  for (const d of nodes) {
    const v = draw(dr, d);
    if (typeof v === "string") {
      // words that follow words are one text, as a document read back from HTML has them
      const last = out.length - 1;
      if (typeof out[last] === "string") out[last] += v;
      else if (v) out.push(v);
      continue;
    }
    v.key = d.id;
    v.attrs["data-xtmd-site"] = d.id;
    v.attrs["data-xtmd-widget"] = d.widget;
    out.push(v);
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
      event: (name, args = []) => {
        dr.page.event(d.id, name, args);
        dr.after?.();
      },
      write: (input, value) => {
        dr.page.write(d.id, input, value);
        dr.after?.();
      },
      call: (input, args) => dr.page.call(d.id, input, args),
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
    page: dr.page,
    tokens: dr.tokens,
    uid: dr.uid,
    event: (name, args = []) => {
      dr.page.event(d.id, name, args);
      dr.after?.();
    },
    write: (input, value) => {
      dr.page.write(d.id, input, value);
      dr.after?.();
    },
    call: (input, args) => dr.page.call(d.id, input, args),
    children: (nodes) => children(dr, nodes),
    layout: () => dr.layout(),
  };
  try {
    return maker(d, ctx);
  } catch (f) {
    dr.page.errors.push(f);
    return notice(`${d.widget.split("/").pop()} couldn't be drawn: ${f instanceof Error ? f.message : String(f)}`);
  }
}
