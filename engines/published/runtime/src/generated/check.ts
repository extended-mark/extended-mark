// Written by xtmd-schemagen from schemas/check.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Block } from "./blocks.js";
import type { Expr } from "./ir.js";

/** What the compiler gives: its diagnostics, and the page's Markdown with its starting values. */
export interface CaseExpect {
  diagnostics: Diagnostic[];
  /** The page's Markdown with its starting values, as `xtmd md` writes it, before any step. */
  markdown?: MarkdownExpected;
}

/** A compiler case: a source, the diagnostics it gives, and the page's Markdown with its starting values. */
export interface Case {
  about?: string;
  /** What the compiler gives: its diagnostics, and the page's Markdown with its starting values. */
  expect: CaseExpect;
  /** `group.name`, unique. */
  id: string;
  /** Packs to build with, beside the case. */
  packs?: string[];
  /** The .xtmd or .xtmdpack file, beside the case. */
  source: string;
}

export type DiagnosticSeverity = "error" | "warning";

/** A diagnostic, by its code and where it starts. Messages are never compared. */
export interface Diagnostic {
  code: string;
  /** Counted in characters, from 1. */
  column: number;
  line: number;
  severity?: DiagnosticSeverity;
}

export type MarkdownExpected =
  | { blocks: Block[] }
  | { has: Block[] }
  | { file: string };

export interface Mounted {
  props: { [key: string]: Expr };
  /** Each slot's children, by site, in order. */
  slots?: { [key: string]: string[] };
  widget: string;
}

export type PageTheme = "LIGHT" | "DARK";

export type PageWidth = "NARROW" | "MEDIUM" | "WIDE";

/** What the host tells the page about where it is read. */
export interface Page {
  now?: string;
  theme?: PageTheme;
  today?: string;
  width?: PageWidth;
}
