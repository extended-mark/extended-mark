// The live page (/live): @extended-mark/live's XtmdLiveDoc, which compiles the text in the box with xtmd.wasm as it
// changes. A draft that doesn't compile leaves the last page that did.

import { XtmdLiveDoc, type LiveDocProps } from "@extended-mark/live/dom";
import { themeNow } from "./host.js";
import { report } from "./report.js";
import coffee from "../pages/coffee.xtmd";
import counter from "../examples/counter.xtmd";
import triangle from "../pages/triangle.xtmd";

const TEXTS: Record<string, string> = { triangle, coffee, counter };

function el<T extends HTMLElement = HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`the page has no #${id}`);
  return found as T;
}

const source = el<HTMLTextAreaElement>("source");
const picker = el<HTMLSelectElement>("picker");
const status = el("status");

function say(state: "ok" | "errors" | "pending", text: string): void {
  status.dataset.state = state;
  status.textContent = text;
}

let props: LiveDocProps = {
  source: (source.value = triangle),
  wasm: "/xtmd.wasm",
  page: { theme: themeNow() },
  onPage: () => say("ok", "Compiled here, in your browser"),
  onError: (e) => {
    if (e.kind === "compile") say("errors", e.message.replace(/^page\.xtmd doesn't compile/, "Doesn't compile"));
    else if (e.kind === "load") {
      say("errors", e.message);
      report("compiler-load", e.message);
    }
  },
};
const live = new XtmdLiveDoc(el("page"), props);

let timer: ReturnType<typeof setTimeout> | undefined;
function edited(wait: number): void {
  clearTimeout(timer);
  timer = setTimeout(() => {
    say("pending", "Compiling…");
    props = { ...props, source: source.value };
    live.update(props);
  }, wait);
}
source.addEventListener("input", () => edited(150));
picker.addEventListener("change", () => {
  source.value = TEXTS[picker.value];
  edited(0);
});
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => live.update((props = { ...props, page: { theme: themeNow() } })));
