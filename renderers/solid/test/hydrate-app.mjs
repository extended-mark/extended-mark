// A page the engine built, taken over from the HTML a server wrote.
import { createComponent } from "solid-js";
import { hydrate } from "solid-js/web";
import { page } from "#pages";

hydrate(() => createComponent(page("v1").of, { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }), document.getElementById("app"));
