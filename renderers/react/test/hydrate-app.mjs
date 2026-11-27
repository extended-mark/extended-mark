// A page the engine built, taken over from the HTML a server wrote.
import { createElement as h } from "react";
import { hydrateRoot } from "react-dom/client";
import Counter from "./built/v1.js";

hydrateRoot(document.getElementById("app"), h(Counter, { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }));
