// A page the engine built, taken over from the HTML a server wrote.
import { hydrate } from "svelte";
import { page } from "#pages";

hydrate(page("v1").of, { target: document.getElementById("app"), props: { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } } });
