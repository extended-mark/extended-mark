// Written by xtmd-schemagen from schemas/blocks.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

/** The format's version. */
export const VERSION = 1;

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

/** A Markdown block. */
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

/** A page as Markdown blocks. */
export interface Blocks {
  blocks: Block[];
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
