"use client";
// A page in React's own elements, whichever way it is run: part of the host's React tree, rendered on a server as it is
// on the client, and kept by key across redraws, so focus and what is being typed survive a change. Listeners, live DOM
// properties and the hooks of widgets that draw on their element (a canvas, a plane) are attached to the real elements.

import { createElement, useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, useSyncExternalStore, type ComponentType, type CSSProperties, type ReactElement, type ReactNode, type Ref } from "react";
import { composing, type Child, type Held, type HeldProps, type VNode, type WidgetProps } from "@extended-mark/widgets";

/** What a React component that draws a widget is given. */
export interface ReactWidgetProps extends WidgetProps {
  /** An input's widgets: a slot's, or a rich_text's. */
  slot(input: string): ReactNode;
}

// ---- elements, the React way

/** Attributes React spells in camelCase. */
const ATTRS: Record<string, string> = { class: "className", for: "htmlFor", tabindex: "tabIndex", readonly: "readOnly", colspan: "colSpan", rowspan: "rowSpan", maxlength: "maxLength", spellcheck: "spellCheck", autocomplete: "autoComplete", contenteditable: "contentEditable", popover: "popover" };

function camel(s: string): string {
  return s.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());
}

/** An attribute's React name: data- and aria- as they are, SVG's kebab-case attributes camelCase. */
function attrName(k: string, svg: boolean): string {
  if (ATTRS[k]) return ATTRS[k];
  if (k.startsWith("data-") || k.startsWith("aria-")) return k;
  return svg && k.includes("-") ? camel(k) : k;
}

/** A CSS property's React name: custom properties as they are, the rest camelCase. */
function styleName(k: string): string {
  return k.startsWith("--") ? k : camel(k);
}

const FORM = new Set(["input", "textarea", "select"]);

/** Whether an element needs a component of its own: listeners, live properties, or a hook. */
function live(v: VNode): boolean {
  return !!(v.hook || (v.on && Object.keys(v.on).length) || (v.props && ("value" in v.props || "checked" in v.props || "indeterminate" in v.props)));
}

function reactProps(v: VNode, ref?: Ref<Element>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, x] of Object.entries(v.attrs)) out[attrName(k, !!v.svg)] = x;
  if (v.style) out.style = Object.fromEntries(Object.entries(v.style).map(([k, x]) => [styleName(k), x]));
  for (const [k, x] of Object.entries(v.props ?? {})) {
    // a form control's value and checked are the live element's: React only writes the first
    if (k === "value" && FORM.has(v.tag)) out.defaultValue = x;
    else if (k === "checked") out.defaultChecked = x;
    else if (k !== "indeterminate") out[k] = x;
  }
  if (v.key !== undefined) out.key = v.key;
  if (ref) out.ref = ref;
  // a style element's text is the stylesheet, not a child to escape
  if (v.tag === "style") out.dangerouslySetInnerHTML = { __html: v.children.map((c) => (typeof c === "string" ? c : "")).join("") };
  return out;
}

function kids(v: VNode): ReactNode[] {
  if (v.tag === "style" || (v.tag === "textarea" && v.props && "value" in v.props)) return [];
  return v.children.map(toReact);
}

/** An element with listeners, live properties or a hook: a component, so it can reach its DOM node. */
function Live({ v }: { v: VNode }): ReactElement {
  const ref = useRef<Element>(null);
  const latest = useRef(v);
  latest.current = v;
  const mounted = useRef(false);
  const names = Object.keys(v.on ?? {}).sort().join(" ");
  // listeners on the element itself, as the DOM gives them, each calling the latest render's
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !names) return;
    const added = names.split(" ").map((n) => {
      const f = (e: Event) => latest.current.on?.[n]?.(e);
      el.addEventListener(n, f);
      return [n, f] as const;
    });
    return () => added.forEach(([n, f]) => el.removeEventListener(n, f));
  }, [names]);
  // live properties: what the page shows wins, except while a person is composing text in it
  useLayoutEffect(() => {
    const el = ref.current as (HTMLInputElement & HTMLTextAreaElement) | null;
    const p = v.props;
    if (!el || !p) return;
    if ("value" in p && !composing(el) && el.value !== String(p.value ?? "")) {
      const focused = el.ownerDocument.activeElement === el;
      let sel: [number | null, number | null] = [null, null];
      try {
        sel = [el.selectionStart, el.selectionEnd];
      } catch {
        // a range input has no caret
      }
      el.value = String(p.value ?? "");
      if (focused && sel[0] !== null && sel[1] !== null) {
        try {
          el.setSelectionRange(Math.min(sel[0], el.value.length), Math.min(sel[1], el.value.length));
        } catch {
          // not every input keeps a caret
        }
      }
    }
    if ("checked" in p) el.checked = !!p.checked;
    if ("indeterminate" in p) el.indeterminate = !!p.indeterminate;
  });
  // the widget's hook: once in the document, after each render it stays through, and as it leaves
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (!mounted.current) {
      mounted.current = true;
      latest.current.hook?.mount?.(el);
    } else latest.current.hook?.update?.(el);
  });
  useLayoutEffect(() => {
    const el = ref.current;
    return () => {
      mounted.current = false;
      if (el) latest.current.hook?.unmount?.(el);
    };
  }, []);
  return createElement(v.tag, reactProps(v, ref), ...kids(v));
}

/** One of the widgets' plain elements as React's. */
export function toReact(c: Child): ReactNode {
  if (typeof c === "string") return c;
  if (c.component) {
    const { of, props, children } = c.component;
    return createElement(c.tag, reactProps(c), createElement(of as ComponentType<ReactWidgetProps>, { ...props, slot: (input) => children(input).map(toReact) }));
  }
  if (live(c)) return createElement(Live, { v: c, key: c.key });
  return createElement(c.tag, reactProps(c), ...kids(c));
}

// ---- a held page

/** What an element around a page is given. */
export interface Around {
  id?: string;
  className?: string;
  style?: CSSProperties;
}

/** What a page's ids start with: React's own, which is the same on a server as in the browser. */
export function useUid(): string {
  return `xtmd${useId().replace(/\W/g, "")}-`;
}

/** A held page, followed: rendered at each change, its props its host's, its handle what `handle` makes. */
export function useHeld<P extends HeldProps, D extends Held<P>, H>(doc: D, props: P, ref: Ref<H | null> | undefined, handle: (doc: D) => H, { id, className, style }: Around): ReactElement {
  useSyncExternalStore(
    (f) => doc.subscribe(f),
    () => doc.version,
    () => doc.version,
  );
  // after every render: the page follows its props
  useEffect(() => {
    doc.update(props);
  });
  useImperativeHandle<H | null, H | null>(ref, () => handle(doc), [doc]);
  const root = doc.render();
  if (id) root.attrs.id = id;
  if (className) root.attrs.class = `${root.attrs.class} ${className}`;
  const el = toReact(root) as ReactElement;
  return style ? createElement("div", { style }, el) : el;
}
