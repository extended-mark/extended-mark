// The test host for <XtmdDoc>: drawn by React itself, `$state` controlled by the host, with the
// controls the shared doc steps use (testing/browser.mjs, docSteps).
import { createElement as h, useState, useRef, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { XtmdDoc } from "@extended-mark/react";

const heard = { states: [], errors: [], calls: [] };
let controls;

const components = {
  "core/press": ({ node, event }) => h("button", { className: "mine", onClick: () => event("click") }, node.inputs.label.text),
  "core/heading": ({ slot }) => h("section", { className: "title" }, slot("value")),
};

function App() {
  const [which, setIr] = useState("v1");
  const [theme, setTheme] = useState("LIGHT");
  const [unit, setUnit] = useState("items");
  const [state, setState] = useState(undefined);
  const [shown, setShown] = useState(true);
  const ref = useRef(null);
  controls = { setIr, setTheme, setUnit, setState, unmount: () => setShown(false), ref };
  if (!shown) return null;
  return h(XtmdDoc, {
    ref,
    ir: window.__IR[which],
    state,
    components: window.__mine ? components : undefined,
    page: { theme },
    externs: { app: { unit: { text: unit } } },
    onStateChange: (s) => {
      heard.states.push(s);
      setState(s);
    },
    onError: (e) => heard.errors.push(e),
    onCalls: (c) => heard.calls.push(c),
  });
}

createRoot(document.getElementById("app")).render(h(StrictMode, null, h(App)));

window.__test = {
  heard,
  handle: () => controls.ref.current,
  setState: (s) => controls.setState(s),
  setTheme: (t) => controls.setTheme(t),
  setIr: (w) => controls.setIr(w),
  setUnit: (u) => controls.setUnit(u),
  unmount: () => controls.unmount(),
};
