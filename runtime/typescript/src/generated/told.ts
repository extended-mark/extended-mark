// Written by xtmd-schemagen from schemas/told.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Input } from "./tree.js";

/** The format's version. */
export const VERSION = 1;

/** A rule that failed: a `require`'s message, or a contract's code, and where. */
export interface Broken {
  code?: string;
  rule?: string;
  site?: string;
}

export interface CallCall {
  args: Input[];
  name: string;
}

/** A fn of the host's the page called, with its arguments. */
export interface Call {
  call: CallCall;
}

export interface Outcome {
  failures?: Broken[];
  refused?: Broken[];
}

/** What a page tells its host, since it was last taken. */
export interface Told {
  /** The host's fns the page called, in order. */
  calls?: Call[];
  failures?: Broken[];
  /** Writes that were refused: nothing changed. */
  refused?: Broken[];
  /** The `warn`s found false. */
  warnings?: Broken[];
}
