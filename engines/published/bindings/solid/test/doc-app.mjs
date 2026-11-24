// The test host for <XtmdDoc>: drawn by Solid itself, `$state` controlled by the host, with the
// controls the shared doc steps use (testing/browser.mjs, docSteps).
import { createComponent, createSignal, Show } from "solid-js";
import { Dynamic, render } from "solid-js/web";
import { XtmdDoc } from "@extended-mark/solid";

const heard = { states: [], errors: [], calls: [] };
const [which, setIr] = createSignal("v1");
const [theme, setTheme] = createSignal("LIGHT");
const [unit, setUnit] = createSignal("items");
const [state, setState] = createSignal(undefined);
const [shown, setShown] = createSignal(true);
let handle;

const components = {
  "core/press": (p) => createComponent(Dynamic, { component: "button", class: "mine", "on:click": () => p.event("click"), get children() {
    return p.node.inputs.label.text;
  } }),
  "core/heading": (p) => createComponent(Dynamic, { component: "section", class: "title", get children() {
    return p.slot("value");
  } }),
};

render(
  () =>
    createComponent(Show, {
      get when() {
        return shown();
      },
      get children() {
        return createComponent(XtmdDoc, {
          ref: (h) => (handle = h),
          get ir() {
            return window.__IR[which()];
          },
          get state() {
            return state();
          },
          components: window.__mine ? components : undefined,
          get page() {
            return { theme: theme() };
          },
          get externs() {
            return { app: { unit: { text: unit() } } };
          },
          onStateChange: (s) => {
            heard.states.push(s);
            setState(s);
          },
          onError: (e) => heard.errors.push(e),
          onCalls: (c) => heard.calls.push(c),
        });
      },
    }),
  document.getElementById("app"),
);

window.__test = {
  heard,
  handle: () => handle,
  setState: (s) => setState(s),
  setTheme: (t) => setTheme(t),
  setIr: (w) => setIr(w),
  setUnit: (u) => setUnit(u),
  unmount: () => setShown(false),
};
