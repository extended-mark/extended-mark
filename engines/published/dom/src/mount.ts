// A page drawn into an element: its widgets' elements followed onto it by id after every change, so each
// lives as long as its widget does, and focus, the caret and what is being typed survive a redraw. The
// plane's layout runs after every change, with sizes read from the DOM.

import { Num } from "@extended-mark/runtime/num";
import { Page, type Node as Drawn, type PageOptions, type ir } from "@extended-mark/runtime";
import { Patcher, Tokens, act, children, composing, h, layoutPlanes, styleText, webFns, widgets as ours, type Child, type Component, type Drawing, type Host, type Measure } from "@extended-mark/widgets";
import type { MountOptions, Mounted, OwnWidget } from "./api.js";

const SVG = "http://www.w3.org/2000/svg";
let mounts = 0;

/** How elements are made and changed here: the DOM's own way. */
const dom: Host<Node> = {
  live: true,
  createElement: (tag, svg) => (svg ? document.createElementNS(SVG, tag) : document.createElement(tag)),
  createText: (text) => document.createTextNode(text),
  setText: (node, text) => {
    if (node.nodeValue !== text) node.nodeValue = text;
  },
  setAttribute: (el, name, value) => (el as Element).setAttribute(name, value),
  removeAttribute: (el, name) => (el as Element).removeAttribute(name),
  setStyle: (el, prop, value) => (el as HTMLElement).style.setProperty(prop, value),
  removeStyle: (el, prop) => (el as HTMLElement).style.removeProperty(prop),
  // what is typed stays while a person composes it, and a value that is there already is never set again:
  // that would move the caret
  setProperty: (el, name, value) => {
    const live = el as unknown as Record<string, unknown>;
    if (name === "value" && composing.has(el)) return;
    if (live[name] !== value) live[name] = value;
  },
  listen(el, event, f) {
    el.addEventListener(event, f);
    return () => el.removeEventListener(event, f);
  },
  insertBefore: (parent, child, before) => void parent.insertBefore(child, before),
  removeChild: (_parent, child) => (child as ChildNode).remove(),
};

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

/** A host's own element for a widget, its slots' widgets followed into where it puts them. */
function own(el: Node, c: Component): { update(c: Component): void; destroy(): void } {
  let now = c;
  const slots = new Map<Element, { input: string; patcher: Patcher<Node> }>();
  const follow = () => {
    for (const { input, patcher } of slots.values()) patcher.render(holder(now.children(input)));
  };
  const holder = (kids: Child[]) => ({ ...h("xtmd-slot", undefined, {}, kids), style: { display: "contents" } });
  const made = (c.of as OwnWidget)(el as HTMLElement, c.props, (container, input) => {
    if (!slots.has(container)) slots.set(container, { input, patcher: new Patcher<Node>(dom, container) });
    follow();
  });
  return {
    update(next) {
      now = next;
      made.update(next.props);
      follow();
    },
    destroy() {
      for (const s of slots.values()) s.patcher.destroy();
      made.destroy?.();
    },
  };
}
dom.component = own;

export function mount(page: Page, root: HTMLElement, opts: MountOptions = {}): Mounted {
  const registry = { ...ours, ...(opts.widgets ?? {}) };
  const components = opts.own;
  // an extern widget nobody registered shows its Markdown instead (page 6): the page draws it
  // with the blocks we do draw, and reports it in page.missing()
  page.setSupplies((widget) => Object.hasOwn(registry, widget) || !!components && Object.hasOwn(components, widget));
  const tokens = new Tokens(page);
  injectStyle(root, tokens);
  let live = true;
  let rendering = false;
  let again = false;
  let settling = false;

  root.classList.add("xtmd-root", tokens.sheet().name);
  root.setAttribute("data-xtmd-root", "");
  root.replaceChildren();

  const drawing: Drawing = {
    registry,
    components,
    failed: opts.failed,
    tokens,
    uid: `xtmd${++mounts}-`,
    kept: new WeakMap<Drawn, Child>(),
    node: (id) => page.node(id),
    event: (id, name, args) => page.event(id, name, args),
    write: (id, input, value) => page.write(id, input, value),
    call: (id, input, args) => page.call(id, input, args),
    // it is laid out after every change; only a size that changed by itself asks for more
    layout: (_id, over) => {
      if (over) settle();
    },
    fail: (f) => page.errors.push(f),
  };
  const patcher = new Patcher<Node>(dom, root, true);

  function element(id: string): HTMLElement | undefined {
    return root.querySelector<HTMLElement>(`[data-xtmd-site="${CSS.escape(id)}"]`) ?? undefined;
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
        patcher.renderAll(children(drawing, page.tree().nodes));
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
      const el = element(s.id);
      return { width: Num.int(Math.max(0, el?.offsetWidth ?? 0)), height: Num.int(Math.max(0, el?.offsetHeight ?? 0)) };
    },
    planeWidth(p) {
      const el = element(p.id);
      const view = el?.querySelector<HTMLElement>("[data-part='plane.view']");
      return Num.int(Math.max(0, Math.round((view ?? el)?.clientWidth ?? 0)));
    },
  };

  function settle(): void {
    if (!live || settling) return;
    settling = true;
    try {
      layoutPlanes(page, measure);
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
    element,
    act: (id, action) => act(root, page, id, action),
    layout: settle,
    unmount(): void {
      if (!live) return;
      off();
      live = false;
      patcher.destroy();
      root.classList.remove("xtmd-root", tokens.sheet().name);
      root.removeAttribute("data-xtmd-root");
      root.removeAttribute("data-xtmd-theme");
      root.replaceChildren();
    },
  };
}

/** Makes the page from an IR file, with the web's value fns under the host's own, and mounts it. */
export function run(irFile: ir.File | ir.Module | string, root: HTMLElement, opts: PageOptions & MountOptions = {}): Mounted {
  const { widgets, own, measure, failed, ...pageOpts } = opts;
  const file = typeof irFile === "string" ? (JSON.parse(irFile) as ir.File) : irFile;
  const page = new Page(file, { ...pageOpts, fns: { ...webFns(), ...(pageOpts.fns ?? {}) } });
  return mount(page, root, { widgets, own, measure, failed });
}
