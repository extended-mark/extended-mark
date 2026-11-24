// Texts and durations as they are shown and cut, by code points. A text is a sequence of Unicode scalar values, as
// Rust's `str`: lengths, positions and order count code points, not UTF-16 units.

import { Num, NumError } from "./num.js";

const THOUSAND = Num.int(1000);

/** Whole seconds as `2s`, anything else as `250ms`. */
export function showDuration(ms: Num): string {
  try {
    const s = ms.div(THOUSAND);
    if (s.isInt()) {
      return `${s.show()}s`;
    }
  } catch (e) {
    if (!(e instanceof NumError)) {
      throw e;
    }
  }
  return `${ms.show()}ms`;
}

// ---- Unicode text, by code points

let lastText = "";
let lastPlain = true;

/** Whether a text has no character above U+FFFF, so its UTF-16 units are its characters. */
function plain(t: string): boolean {
  if (t !== lastText) {
    lastText = t;
    lastPlain = !/[\ud800-\udfff]/.test(t);
  }
  return lastPlain;
}

/** The number of characters (Unicode scalar values) in a text. */
export function textLength(t: string): number {
  if (plain(t)) {
    return t.length;
  }
  let n = 0;
  for (const _ of t) {
    n += 1;
  }
  return n;
}

export function chars(t: string): string[] {
  return plain(t) ? t.split("") : Array.from(t);
}

export function sliceChars(t: string, start: number, count: number): string {
  if (plain(t)) {
    return start >= t.length ? "" : t.substring(start, start + Math.min(count, t.length));
  }
  let out = "";
  let k = 0;
  for (const c of t) {
    if (k >= start + count) {
      break;
    }
    if (k >= start) {
      out += c;
    }
    k += 1;
  }
  return out;
}

/** Unicode's White_Space, as Rust's `char::is_whitespace` and `str::trim`. */
function isWhitespace(c: number): boolean {
  return (
    (c >= 0x09 && c <= 0x0d) ||
    c === 0x20 ||
    c === 0x85 ||
    c === 0xa0 ||
    c === 0x1680 ||
    (c >= 0x2000 && c <= 0x200a) ||
    c === 0x2028 ||
    c === 0x2029 ||
    c === 0x202f ||
    c === 0x205f ||
    c === 0x3000
  );
}

export function trim(t: string): string {
  let a = 0;
  let b = t.length;
  while (a < b && isWhitespace(t.charCodeAt(a))) {
    a += 1;
  }
  while (b > a && isWhitespace(t.charCodeAt(b - 1))) {
    b -= 1;
  }
  return t.slice(a, b);
}

export function digits(s: string): boolean {
  if (s.length === 0) {
    return false;
  }
  for (let k = 0; k < s.length; k++) {
    const c = s.charCodeAt(k);
    if (c < 48 || c > 57) {
      return false;
    }
  }
  return true;
}
