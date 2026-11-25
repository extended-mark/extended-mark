// Written by xtmd-schemagen from schemas/state.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Input } from "./tree.js";

/** The format's version. */
export const VERSION = 1;

/** The page's data: what a host keeps to put a page back as it was. Plain values only. */
export interface Data {
  /** Each widget's own variables, by the place it is drawn at. */
  own?: { [key: string]: { [key: string]: Input } };
  /** `$state`, by field. */
  state: { [key: string]: Input };
}
