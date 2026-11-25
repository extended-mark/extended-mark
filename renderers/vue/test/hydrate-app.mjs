// A page the engine built, taken over from the HTML a server wrote.
import { createSSRApp, h } from "vue";
import { page } from "#pages";

createSSRApp({ render: () => h(page("v1").of, { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }) }).mount("#app");
