// A page drawn into an element: its tree reconciled by id after every change, so each widget's
// element lives as long as the widget does, and focus, the caret and what is being typed survive
// a redraw. The plane's layout runs after every change, with sizes read from the DOM.

import { Num, Page, layout as runLayout, type Measure, type Node as Drawn, type PageOptions, type ir } from "@extended-mark/runtime";
import type { Action, Mounted, MountOptions, Widget, WidgetContext, WidgetView } from "./api.js";
import { giveKit, newUid, tokensOf, type Kit } from "./kit.js";
import type { Tokens } from "./tokens.js";
import { webFns } from "./fns.js";
import { css as styleText } from "./style.js";
import { widgets as ours } from "./widgets/index.js";
import { broken, failedView, notice, unknownWidget } from "./widgets/notice.js";

interface Entry {
  id: string;
  widget: string;
  d: Drawn;
  view: WidgetView;
  ctx: WidgetContext;
}

const styled = new WeakSet<Node>();
const sheets = new WeakMap<Node, Set<string>>();

/** Puts the stylesheet in once, and each page's tokens: in the document's head, or in the shadow root
 * the page is in. */
function injectStyle(root: HTMLElement, tokens: Tokens): void {
  const host = root.getRootNode();
  const target: Node = host instanceof ShadowRoot ? host : root.ownerDocument;
  const parent = host instanceof ShadowRoot ? host : root.ownerDocument.head;
  const add = (mark: string, css: string, first: boolean) => {
    const style = root.ownerDocument.createElement("style");
    style.setAttribute(mark, "");
    style.textContent = css;
    if (first) parent.prepend(style);
    else parent.append(style);
  };
  if (!styled.has(target)) {
    styled.add(target);
    if (!parent.querySelector?.("style[data-xtmd-dom]")) add("data-xtmd-dom", styleText, true);
  }
  let has = sheets.get(target);
  if (!has) sheets.set(target, (has = new Set()));
  const sheet = tokens.sheet();
  if (has.has(sheet.name)) return;
  has.add(sheet.name);
  add("data-xtmd-tokens", sheet.css, false);
}

/** What has focus inside the page, and the caret in it, before a redraw moves anything. */
function focusOf(root: HTMLElement): (() => void) | undefined {
  const a = root.ownerDocument.activeElement as HTMLElement | null;
  if (!a || a === root || !root.contains(a)) return undefined;
  const typed = a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement ? a : undefined;
  let sel: [number | null, number | null, "forward" | "backward" | "none" | null] | undefined;
  try {
    if (typed) sel = [typed.selectionStart, typed.selectionEnd, typed.selectionDirection];
  } catch {
    // a range input has no caret
  }
  return () => {
    if (!a.isConnected || root.ownerDocument.activeElement === a) return;
    a.focus({ preventScroll: true });
    if (typed && sel && sel[0] !== null && sel[1] !== null) {
      try {
        typed.setSelectionRange(sel[0], sel[1], sel[2] ?? undefined);
      } catch {
        // not every input keeps a caret
      }
    }
  };
}

export function mount(page: Page, root: HTMLElement, opts: MountOptions = {}): Mounted {
  const registry: Record<string, Widget> = { ...ours, ...(opts.widgets ?? {}) };
  // an extern widget nobody registered shows its Markdown instead (page 6): the page draws it
  // with the blocks we do draw, and reports it in page.missing()
  page.setSupplies((widget) => Object.hasOwn(registry, widget));
  const tokens = tokensOf(page);
  injectStyle(root, tokens);
  const uid = newUid();
  const containers = new Map<Element, Map<string, Entry>>();
  const byId = new Map<string, Entry>();
  let live = true;
  let rendering = false;
  let again = false;
  let settling = false;

  root.classList.add("xtmd-root", tokens.sheet().name);
  root.setAttribute("data-xtmd-root", "");
  root.replaceChildren();

  const kitFor: Kit = {
    tokens,
    uid,
    layout: () => settle(),
    view: (id) => byId.get(id)?.view,
    release: (el) => release(el),
  };

  function mark(e: Entry, d: Drawn): void {
    byId.set(d.id, e);
    const el = e.view.el;
    if (!(el instanceof Element)) return;
    if (el.getAttribute("data-xtmd-site") !== d.id) el.setAttribute("data-xtmd-site", d.id);
    if (el.getAttribute("data-xtmd-widget") !== d.widget) el.setAttribute("data-xtmd-widget", d.widget);
  }

  function make(d: Drawn): Entry {
    const e = { id: d.id, widget: kindOf(d), d } as Entry;
    e.ctx = {
      page,
      event: (name, args = []) => page.event(e.id, name, args),
      write: (input, value) => page.write(e.id, input, value),
      call: (input, args) => page.call(e.id, input, args),
      children,
      token: (v) => tokens.value(v),
    };
    giveKit(e.ctx, kitFor);
    const maker = d.failed !== undefined ? (opts.failed ?? notice) : (registry[d.widget] ?? unknownWidget);
    try {
      e.view = maker(d, e.ctx);
    } catch (f) {
      page.errors.push(f);
      e.view = failedView(d, f);
    }
    return e;
  }

  function update(e: Entry, d: Drawn): void {
    // the same node: nothing it shows changed
    if (e.d === d) return;
    e.d = d;
    try {
      e.view.update(d);
    } catch (f) {
      page.errors.push(f);
      const failed = failedView(d, f);
      drop(e, true);
      e.view = failed;
    }
  }

  /** Forgets the views drawn into every container inside an element, the element's own included. */
  function release(el: Element): void {
    for (const [c, m] of [...containers]) {
      if (!containers.has(c) || !el.contains(c)) continue;
      containers.delete(c);
      for (const e of m.values()) forget(e);
    }
  }

  function forget(e: Entry): void {
    if (e.view.el instanceof Element) release(e.view.el);
    try {
      e.view.destroy?.();
    } catch (f) {
      page.errors.push(f);
    }
    if (byId.get(e.id) === e) byId.delete(e.id);
  }

  /** Takes a widget off the page: its element, its slots' views, and whatever it holds. */
  function drop(e: Entry, keepEl = false): void {
    forget(e);
    if (!keepEl) e.view.el.remove();
  }

  /** What is drawn: a widget that failed is drawn by another, and is another when it draws again. */
  function kindOf(d: Drawn): string {
    return d.failed === undefined ? d.widget : `${d.widget} failed`;
  }

  function children(container: Element, nodes: readonly Drawn[]): void {
    let map = containers.get(container);
    if (!map) containers.set(container, (map = new Map()));
    const seen = new Map<string, Entry>();
    const els: ChildNode[] = [];
    for (const d of nodes) {
      let e = map.get(d.id);
      if (e && (e.widget !== kindOf(d) || broken.has(e.view))) {
        drop(e);
        map.delete(d.id);
        e = undefined;
      }
      if (e) {
        // a widget may change its element (h2 to h3): the new one takes the old one's place
        const before = e.view.el;
        update(e, d);
        if (e.view.el !== before) before.replaceWith(e.view.el);
      } else e = make(d);
      mark(e, d);
      seen.set(d.id, e);
      els.push(e.view.el);
    }
    for (const [k, e] of map) if (!seen.has(k)) drop(e);
    containers.set(container, seen);
    // in the tree's order, moving only what is out of place, so a focused field stays put
    let cursor: ChildNode | null = container.firstChild;
    for (const el of els) {
      if (el === cursor) cursor = cursor.nextSibling;
      else container.insertBefore(el, cursor);
    }
  }

  function render(): void {
    if (!live) return;
    if (rendering) {
      again = true;
      return;
    }
    rendering = true;
    const refocus = focusOf(root);
    try {
      const theme = tokens.theme().toLowerCase();
      if (root.getAttribute("data-xtmd-theme") !== theme) root.setAttribute("data-xtmd-theme", theme);
      for (let k = 0; k < 4; k++) {
        again = false;
        children(root, page.tree().nodes);
        if (!again) break;
      }
    } finally {
      rendering = false;
    }
    refocus?.();
  }

  /** The default measure: each spot's box on the page, which the plane's transform never changes. */
  const measure: Measure = opts.measure ?? {
    spot(s) {
      const el = byId.get(s.id)?.view.el as HTMLElement | undefined;
      return { width: Num.int(Math.max(0, el?.offsetWidth ?? 0)), height: Num.int(Math.max(0, el?.offsetHeight ?? 0)) };
    },
    planeWidth(p) {
      const el = byId.get(p.id)?.view.el as HTMLElement | undefined;
      const view = el?.querySelector?.("[data-part='plane.view']") as HTMLElement | null | undefined;
      return Num.int(Math.max(0, Math.round((view ?? el)?.clientWidth ?? 0)));
    },
  };

  function settle(): void {
    if (!live || settling) return;
    settling = true;
    try {
      runLayout(page, measure);
    } catch (f) {
      page.errors.push(f);
    } finally {
      settling = false;
    }
  }

  const off = page.onChange(() => {
    render();
    settle();
  });
  render();
  settle();

  return {
    page,
    root,
    view: (id) => byId.get(id)?.view,
    act(id: string, action: Action): void {
      const e = byId.get(id);
      if (e?.view.act) {
        try {
          if (e.view.act(action)) return;
        } catch (f) {
          page.errors.push(f);
        }
      }
      if ("event" in action) page.event(id, action.event, action.args ?? []);
      else page.write(id, action.write, action.value);
    },
    layout: settle,
    unmount(): void {
      if (!live) return;
      off();
      for (const m of [...containers.values()]) for (const e of m.values()) drop(e);
      containers.clear();
      byId.clear();
      live = false;
      root.classList.remove("xtmd-root", tokens.sheet().name);
      root.removeAttribute("data-xtmd-root");
      root.removeAttribute("data-xtmd-theme");
      root.replaceChildren();
    },
  };
}

/** Makes the page from an IR file, with the web's value fns under the host's own, and mounts it. */
export function run(irFile: ir.File | ir.Module | string, root: HTMLElement, opts: PageOptions & MountOptions = {}): Mounted {
  const { widgets, measure, failed, ...pageOpts } = opts;
  const file = typeof irFile === "string" ? (JSON.parse(irFile) as ir.File) : irFile;
  const page = new Page(file, { ...pageOpts, fns: { ...webFns(), ...(pageOpts.fns ?? {}) } });
  return mount(page, root, { widgets, measure, failed });
}
