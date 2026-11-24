// The page the browser tests drive: @extended-mark/dom bundled with @extended-mark/runtime, and a few helpers on
// window.xtmd that take and give values in the IR's form, as the checks do.

import { mount, run } from "../dist/index.js";
import { h, widgets } from "@extended-mark/widgets";
import { Num } from "@extended-mark/runtime";

let mounted;

/** A measure with the test engine's fixed sizes: text 8 wide per character and 20 high. */
function fixedMeasure(page) {
  const text = (d) => {
    for (const k of ["value", "label"]) {
      const v = d.inputs[k];
      if (v && "text" in v) return v.text;
    }
    return undefined;
  };
  return {
    spot(s) {
      const kid = Object.values(s.inputs).flatMap((x) => x.nodes ?? [])[0];
      const t = kid ? text(kid) : undefined;
      return { width: Num.int(t === undefined ? 80 : 8 * [...t].length), height: Num.int(20) };
    },
    planeWidth: () => Num.int(640),
  };
}

window.xtmd = {
  start(irText, opts = {}) {
    if (mounted) mounted.unmount();
    const root = document.getElementById("app");
    const file = JSON.parse(irText);
    const extra = {};
    if (opts.fixed) extra.measure = fixedMeasure();
    if (opts.count) {
      // a label that counts how often it is taken away
      window.gone = {};
      extra.widgets = {
        "core/label": (d) => ({ ...h("i"), hook: { unmount: () => (window.gone[d.id] = (window.gone[d.id] ?? 0) + 1) } }),
      };
    }
    if (opts.labels) {
      // a host's own label, in place of ours: an element it makes and follows itself
      extra.own = {
        "core/label": (el, props) => {
          const mark = document.createElement("mark");
          mark.className = "app-label";
          const update = ({ node }) => (mark.textContent = `[${node.inputs.value.text}]`);
          update(props);
          el.append(mark);
          return { update };
        },
      };
    }
    if (opts.without) extra.widgets = Object.fromEntries(opts.without.map((w) => [w, undefined]));
    mounted = run(file, root, { page: opts.page ?? {}, state: opts.state, ...extra });
    window.mounted = mounted;
    return true;
  },
  page: () => mounted.page,
  state: () => mounted.page.state(),
  markdown: () => mounted.page.markdown(),
  outcome: () => mounted.page.takeOutcome(),
  errors: () => mounted.page.errors.map((e) => String(e?.stack ?? e)),
  tree: () => mounted.page.tree(),
  paths() {
    const out = [];
    const walk = (nodes) => {
      for (const n of nodes) {
        out.push({ path: n.id, widget: n.widget });
        Object.values(n.inputs).forEach(inside);
      }
    };
    const inside = (x) => {
      if (x.nodes) walk(x.nodes);
      else if (x.list) x.list.forEach(inside);
      else if (x.some) inside(x.some);
    };
    walk(mounted.page.tree().nodes);
    return out;
  },
  act(path, action) {
    mounted.act(path, action);
  },
  event: (path, name, args = []) => mounted.page.event(path, name, args),
  write: (path, input, value) => mounted.page.write(path, input, value),
  set: (changes) => mounted.page.set(changes),
  setPage: (values) => mounted.page.setPage(values),
  unmount: () => mounted.unmount(),
  widgets: () => Object.keys(widgets),
  mount,
};
