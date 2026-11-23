// What sentences need wherever they are drawn: links and pictures checked before use, and TeX as text.

import type { Value } from "@extended-mark/runtime";
import { field, list, str } from "./values.js";

/** Whether a link may be followed: http, https and mailto, or a relative one. Whitespace and
 * control characters are dropped first, as browsers do, so `java\tscript:` is caught. */
export function safeHref(url: string): string | undefined {
  const plain = url.replace(/[\u0000- \u007f]/g, "");
  if (!plain) return undefined;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(plain);
  if (!scheme) return url.trim();
  return ["http", "https", "mailto"].includes(scheme[1].toLowerCase()) ? url.trim() : undefined;
}

/** Whether a picture may be shown: http, https, relative, or a data: image. */
export function safeSrc(url: string): string | undefined {
  const plain = url.replace(/[\u0000- \u007f]/g, "");
  if (/^data:image\//i.test(plain)) return url.trim();
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(plain);
  if (!scheme) return plain ? url.trim() : undefined;
  return ["http", "https"].includes(scheme[1].toLowerCase()) ? url.trim() : undefined;
}

/** A template's text: its pieces with each hole's shown text between them. */
export function templateText(v: Value | undefined): string {
  const pieces = list(field(v, "pieces"));
  const holes = list(field(v, "holes"));
  return pieces.map((p, k) => str(p) + (holes[k] ? str(field(holes[k], "value")) : "")).join("");
}

/** A latex value's TeX: the text markdown.ts writes between dollars. */
export function texOf(latex: Value | undefined): string {
  return templateText(field(latex, "tex"));
}

/** The id a footnote's mark is reached by. */
export function noteId(uid: string, mark: string): string {
  return `${uid}fn-${encodeURIComponent(mark)}`;
}
