// The plain web's XtmdDoc taking over a page a server rendered: the element holds the HTML of the same page,
// and XtmdDoc goes on from it.
import { XtmdDoc } from "@extended-mark/dom";

const root = document.getElementById("app");
window.__server = root.innerHTML;
const doc = new XtmdDoc(root, { ir: window.__IR.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } });
window.__test = { doc };
