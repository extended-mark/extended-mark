// A page drawn into an element: its widgets' elements followed onto it by id after every change, so each
// lives as long as its widget does, and focus, the caret and what is being typed survive a redraw. The
// plane's layout runs after every change, with sizes read from the DOM.

import { Page, type Node as Drawn, type PageOptions, type ir } from "@extended-mark/runtime";
import { Patcher, Tokens, act, children, focusOf, layoutPlanes, measured, styleText, webFns, widgets as ours, type Child, type Drawing, type Measure } from "@extended-mark/widgets";
import type { MountOptions, Mounted } from "./api.js";
import { dom } from "./own.js";

let mounts = 0;

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
  const measure: Measure = opts.measure ?? measured(element);

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
