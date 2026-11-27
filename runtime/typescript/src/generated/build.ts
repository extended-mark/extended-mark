// Written by xtmd-schemagen from schemas/build.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Module } from "./ir.js";

/** The format's version. */
export const VERSION = 1;

/** What an engine built. */
export interface Built {
  files: File[];
  /** The format's version. */
  version: number;
}

export interface Document {
  ir: Module;
  /** Its place among what is built, with no ending: `sprint`, `docs/sprint`. */
  name: string;
}

/** A file to write: its text, or its bytes. */
export interface File {
  base64?: string;
  /** Where, under the folder built into: `sprint.html`, `assets/page.js`. Parts between slashes, none of them `.` or `..`. */
  path: string;
  text?: string;
}

/** What is built, all of it at once, so an engine can share between documents what is the same in them. */
export interface Job {
  documents: Document[];
  /** The format's version. */
  version: number;
}
