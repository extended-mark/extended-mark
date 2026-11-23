// Written by xtmd-schemagen from schemas/check.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Expr } from "./ir.js";

export interface BlockHeading {
  inline: Inline[];
  level: number;
}

export interface BlockBulletsItems {
  blocks: Block[];
  done?: boolean;
}

export interface BlockBullets {
  items: BlockBulletsItems[];
  ordered: boolean;
  start?: number;
}

export interface BlockCode {
  lang: string;
  src: string;
}

export type BlockTableAlign = "START" | "CENTER" | "END";

export interface BlockTable {
  align: BlockTableAlign[];
  head: Inline[][];
  rows: Inline[][][];
}

export interface BlockFootnote {
  blocks: Block[];
  mark: string;
}

/** A Markdown block, as the runner reads the page. */
export type Block =
  | { paragraph: Inline[] }
  | { heading: BlockHeading }
  | { bullets: BlockBullets }
  | { quote: Block[] }
  | { code: BlockCode }
  | { math: string }
  | { table: BlockTable }
  | { footnote: BlockFootnote }
  | { divider: { [key: string]: never } };

/** A rule that failed: a `require`'s message, or a contract's code, and where. */
export interface Broken {
  code?: string;
  rule?: string;
  site?: string;
}

export interface CallCall {
  args: Expr[];
  name: string;
}

/** An extern fn the step called, with its arguments. */
export interface Call {
  call: CallCall;
}

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

export interface InlineText {
  bold?: boolean;
  italic?: boolean;
  link?: string;
  strike?: boolean;
  value: string;
}

export interface InlineImage {
  alt: string;
  src: string;
}

export type Inline =
  | { text: InlineText }
  | { code: string }
  | { math: string }
  | { image: InlineImage }
  | { note: string }
  | { break: { [key: string]: never } };

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

export interface Outcome {
  failures?: Broken[];
  refused?: Broken[];
}

export type PageTheme = "LIGHT" | "DARK";

export type PageWidth = "NARROW" | "MEDIUM" | "WIDE";

/** What the app tells the page about where it is read. */
export interface Page {
  now?: string;
  theme?: PageTheme;
  today?: string;
  width?: PageWidth;
}
