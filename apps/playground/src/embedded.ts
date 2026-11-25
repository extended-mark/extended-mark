// The embedded page (/embedded): an Extended Mark page as part of a React app. The app brings the IR and holds its
// $state, says what the shop's currency is and whether it is open, hears the orders, and keeps the basket for later.

import { createElement as h, useEffect, useRef, useState, type ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { XtmdDoc, type DocError, type Snapshot, type XtmdDocHandle } from "@extended-mark/react";
import type { Input, Ir } from "@extended-mark/runtime";
import { themeNow } from "./host.js";
import { report } from "./report.js";

type State = Record<string, Input>;
interface Order {
  id: number;
  total: string;
  count: string;
  currency: string;
}

const number = (x: Input | undefined) => (x && "num" in x ? x.num : "0");

/** What is in the basket, from $state as the page gives it. */
function inBasket(state: State | undefined): string[] {
  const lines = state?.lines;
  if (!lines || !("list" in lines)) return [];
  return lines.list.flatMap((l) => {
    if (!("record" in l)) return [];
    const { name, count } = l.record.fields;
    return "text" in name && number(count) !== "0" ? [`${number(count)} × ${name.text}`] : [];
  });
}

function Shop({ ir }: { ir: Ir }): ReactElement {
  const [state, setState] = useState<State | undefined>(undefined);
  const [open, setOpen] = useState(true);
  const [currency, setCurrency] = useState("€");
  const [orders, setOrders] = useState<Order[]>([]);
  const [refused, setRefused] = useState<string | undefined>(undefined);
  const [kept, setKept] = useState<Snapshot | undefined>(undefined);
  const [theme, setTheme] = useState(themeNow);
  const doc = useRef<XtmdDocHandle>(null);
  useEffect(() => setState(doc.current?.state()), []);
  useEffect(() => {
    const dark = matchMedia("(prefers-color-scheme: dark)");
    const change = () => setTheme(themeNow());
    dark.addEventListener("change", change);
    return () => dark.removeEventListener("change", change);
  }, []);
  const basket = inBasket(state);
  return h(
    "div",
    { className: "two" },
    h(
      "section",
      { className: "card", "aria-label": "The page" },
      refused !== undefined && h("p", { className: "refused", role: "status", "data-testid": "refused" }, `Not done: ${refused}.`),
      h(XtmdDoc, {
        ref: doc,
        ir,
        state,
        page: { theme },
        externs: { shop: { currency: { text: currency }, open: { bool: open } } },
        onStateChange: (s: State) => {
          setState(s);
          setRefused(undefined);
        },
        onError: (e: DocError) => {
          // a rule of the page stopped a change, the reader's or the app's: nothing of it was done
          if ((e.kind === "refused" || e.kind === "failed") && e.rule !== undefined) setRefused(e.rule);
          else report(`embedded-${e.kind}`, "message" in e ? e.message : (e.code ?? e.kind));
        },
        onCalls: (calls: { call: { name: string; args: Input[] } }[]) => {
          const made = calls.filter((c) => c.call.name.endsWith("shop.order"));
          setOrders((before) => [...before, ...made.map((c, k) => ({ id: before.length + k + 1, total: number(c.call.args[0]), count: number(c.call.args[1]), currency }))]);
        },
      }),
    ),
    h(
      "div",
      { className: "aside" },
      h(
        "section",
        { className: "card", "aria-labelledby": "gives" },
        h("h2", { id: "gives" }, "The app gives the page"),
        h("p", { className: "note" }, "Values the page can read, and never change."),
        h(
          "div",
          { className: "controls" },
          h("label", null, "The shop is open", h("input", { type: "checkbox", checked: open, "data-testid": "open", onChange: (e: { target: { checked: boolean } }) => setOpen(e.target.checked) })),
          h(
            "label",
            null,
            "Currency",
            h(
              "select",
              { value: currency, "data-testid": "currency", onChange: (e: { target: { value: string } }) => setCurrency(e.target.value) },
              ["€", "$", "£"].map((c) => h("option", { key: c }, c)),
            ),
          ),
        ),
      ),
      h(
        "section",
        { className: "card", "aria-labelledby": "holds" },
        h("h2", { id: "holds" }, "The app holds its state"),
        h("p", { className: "note" }, "In React's own state. Every change the page makes is checked first."),
        basket.length
          ? h("ul", { className: "orders", "data-testid": "holds" }, basket.map((b) => h("li", { key: b }, b)))
          : h("p", { className: "empty", "data-testid": "holds" }, "The basket is empty."),
        h(
          "div",
          { className: "buttons", style: { marginTop: 12 } },
          h("button", { type: "button", className: "button", "data-testid": "keep", onClick: () => setKept(doc.current?.snapshot()) }, "Keep for later"),
          h("button", { type: "button", className: "button", "data-testid": "back", disabled: !kept, onClick: () => kept && doc.current?.restore(kept) }, "Bring it back"),
        ),
      ),
      h(
        "section",
        { className: "card", "aria-labelledby": "hears" },
        h("h2", { id: "hears" }, "The app hears"),
        h("p", { className: "note" }, "The page calls the shop's order, and what happens then is the app's."),
        orders.length
          ? h("ul", { className: "orders", "data-testid": "orders" }, orders.map((o) => h("li", { key: o.id }, h("span", null, `Order ${o.id}, ${o.count} in it`), h("strong", null, `${o.total} ${o.currency}`))))
          : h("p", { className: "empty", "data-testid": "orders" }, "No orders yet."),
      ),
    ),
  );
}

try {
  const r = await fetch("/basket.ir.json");
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
  createRoot(document.getElementById("app")!).render(h(Shop, { ir: (await r.json()) as Ir }));
} catch (e) {
  const why = e instanceof Error ? e.message : String(e);
  document.getElementById("app")!.textContent = `The page couldn't be loaded: ${why}`;
  report("embedded-load", why);
}
