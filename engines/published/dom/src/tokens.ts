// The page's tokens as CSS: each a custom property (`var(--xtmd-color-ink)`), both themes' values in one
// stylesheet, so a theme change is one attribute on the root (`data-xtmd-theme`) and redraws nothing.
// Values come from the IR's tokens.

import type { Input, Page, ir } from "@extended-mark/runtime";
import { field, inner, kase, num, str } from "./values.js";

export type Theme = "LIGHT" | "DARK";

/** A token type's short name in a property: `core/color` is `color`, `acme/brand` is `acme-brand`. */
function short(ty: string): string {
  return (ty.startsWith("core/") ? ty.slice(5) : ty.replace("/", "-")).toLowerCase().replace(/[^a-z0-9-]/g, "-");
}

/** The custom property of a token case: `--xtmd-color-ink`, `--xtmd-space-m`. */
export function tokenVar(ty: string, name: string): string {
  return `--xtmd-${short(ty)}-${name.toLowerCase().replace(/[^a-z0-9-]/g, "-")}`;
}

/** A short name for a text: the same text, the same name, on a server as in a browser. */
function hashOf(text: string): string {
  let h = 0x811c9dc5;
  for (let k = 0; k < text.length; k++) h = Math.imul(h ^ text.charCodeAt(k), 0x01000193);
  return (h >>> 0).toString(36);
}

/** A page's tokens, worked out once. */
interface Sheet {
  /** The class its rules are under. */
  readonly name: string;
  readonly css: string;
}

const sheets = new WeakMap<ir.Module, Sheet>();

export class Tokens {
  private readonly page: Page;
  private readonly named = new Set<string>();
  private readonly types = new Set<string>();

  constructor(page: Page) {
    this.page = page;
    for (const t of tokensOf(page.ir)) {
      this.types.add(t.name);
      for (const name of Object.keys(t.values)) this.named.add(`${t.name}.${name}`);
    }
  }

  /** The page's theme now: core/page.theme, given by the host. */
  theme(): Theme {
    return kase(this.page.given()["core/page.theme"]) === "DARK" ? "DARK" : "LIGHT";
  }

  /** A token case's value: a number, a colour for both themes, or a record. Anything that isn't a token
   * is itself; none is undefined. */
  value(v: Input | undefined): Input | undefined {
    v = inner(v);
    if (!v || !("token" in v || "tokval" in v)) return v;
    try {
      return this.page.token(v);
    } catch {
      return undefined;
    }
  }

  /** A token's number: `space.M` is 8. */
  number(v: Input | undefined): number | undefined {
    return num(this.value(v));
  }

  /** A colour token as CSS: its custom property when the IR names it, else its value. */
  color(v: Input | undefined): string | undefined {
    v = inner(v);
    if (!v) return undefined;
    return this.variable(v) ?? colorOf(this.value(v));
  }

  /** A length token (space, size, radius) as CSS. */
  length(v: Input | undefined): string | undefined {
    v = inner(v);
    if (!v) return undefined;
    const n = this.variable(v) ? undefined : this.number(v);
    return this.variable(v) ?? (n === undefined ? undefined : `${n}px`);
  }

  private variable(v: Input): string | undefined {
    if (!("token" in v) || !this.named.has(v.token)) return undefined;
    const dot = v.token.lastIndexOf(".");
    return `var(${tokenVar(v.token.slice(0, dot), v.token.slice(dot + 1))})`;
  }

  /** Every token's custom property, for both themes, under a class of the page's own. */
  sheet(): Sheet {
    let s = sheets.get(this.page.ir);
    if (!s) {
      const both = { LIGHT: this.properties("LIGHT"), DARK: this.properties("DARK") };
      const body = (p: Record<string, string>) => Object.entries(p).map(([k, v]) => `${k}:${safe(v)};`).join("");
      const name = `xtmd-t${hashOf(body(both.LIGHT) + body(both.DARK))}`;
      const of = (t: Theme) => `.${name}[data-xtmd-theme="${t.toLowerCase()}"]{color-scheme:${t.toLowerCase()};${body(both[t])}}`;
      sheets.set(this.page.ir, (s = { name, css: of("LIGHT") + of("DARK") }));
    }
    return s;
  }

  /** Every token's custom property and its CSS in a theme. */
  private properties(theme: Theme): Record<string, string> {
    const out: Record<string, string> = {};
    for (const t of tokensOf(this.page.ir)) {
      for (const name of Object.keys(t.values)) {
        const v = this.value({ token: `${t.name}.${name}` });
        if (v) for (const [suffix, css] of this.cssOf(v, theme)) out[tokenVar(t.name, name) + suffix] = css;
      }
    }
    return out;
  }

  /** A token's value as CSS: one property, or one per field for a font. */
  private cssOf(v: Input, theme: Theme): [string, string][] {
    if ("num" in v) return [["", `${Number(v.num)}px`]];
    if ("text" in v) return [["", v.text]];
    if (!("record" in v)) return [];
    switch (v.record.type) {
      case "core/color_value":
        return [["", str(field(v, theme === "DARK" ? "dark" : "light"))]];
      case "core/font_value":
        return [
          ["-family", str(field(v, "family"))],
          ["-size", `${num(field(v, "size")) ?? 16}px`],
          ["-weight", str(field(v, "weight")) || "400"],
          ["-line", str(field(v, "line")) || "1.5"],
        ];
      case "core/shadow_value": {
        const tone = this.value(field(v, "tone"));
        const at = (k: string) => `${num(field(v, k)) ?? 0}px`;
        const color = tone && "record" in tone ? str(field(tone, theme === "DARK" ? "dark" : "light")) : str(tone);
        return [["", `${at("x")} ${at("y")} ${at("blur")} ${color || "#0000001f"}`]];
      }
    }
    return [];
  }
}

function tokensOf(m: ir.Module): ir.Token[] {
  return m.declarations.flatMap((d) => ("token" in d.declare ? [d.declare.token] : []));
}

/** A colour's value as CSS, for the theme it is drawn in. */
function colorOf(v: Input | undefined): string | undefined {
  if (!v) return undefined;
  if ("text" in v) return v.text;
  return "record" in v && v.record.type === "core/color_value" ? `light-dark(${str(field(v, "light"))}, ${str(field(v, "dark"))})` : undefined;
}

/** A token's value can't end the rule it is in. */
function safe(css: string): string {
  return css.replace(/[;{}<>]/g, "");
}
