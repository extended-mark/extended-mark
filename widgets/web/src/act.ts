// The reader acting on a drawn widget through its own element, as a person would: what the engine
// checks drive a framework engine with (as @extended-mark/dom's `Mounted.act` does), so an engine is held to
// the runtime through its real DOM. A widget that can't do an action that way goes through the
// page instead.

import type { Page, Input } from "@extended-mark/runtime";
import { acts as controls } from "./widgets/controls.js";
import { acts as blocks } from "./widgets/blocks.js";
import { acts as layout } from "./widgets/layout.js";
import { acts as plane } from "./widgets/plane.js";
import { acts as gfx } from "./widgets/gfx.js";

/** What a reader does to a widget, in the checks' terms. */
export type Action = { event: string; args?: Input[] } | { write: string; value: Input };

/** A widget acting through its element; false when it can't do this that way. */
export type Act = (el: HTMLElement, a: Action, page: Page, id: string) => boolean;

const ACTS: Record<string, Act> = { ...blocks, ...controls, ...layout, ...plane, ...gfx };

/** The reader acting on a widget, by its id, under a root: through its element when it
 * can, else through the page. */
export function act(root: Element, page: Page, id: string, a: Action): void {
  const el = root.querySelector<HTMLElement>(`[data-xtmd-site="${CSS.escape(id)}"]`);
  const widget = el?.getAttribute("data-xtmd-widget") ?? "";
  const f = ACTS[widget];
  if (el && f) {
    try {
      if (f(el, a, page, id)) return;
    } catch (e) {
      page.errors.push(e);
    }
  }
  if ("event" in a) page.event(id, a.event, a.args ?? []);
  else page.write(id, a.write, a.value);
}
