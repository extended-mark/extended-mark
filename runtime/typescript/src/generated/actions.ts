// Written by xtmd-schemagen from schemas/actions.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

import type { Data } from "./state.js";
import type { Input } from "./tree.js";

/** The format's version. */
export const VERSION = 1;

export interface ActionEvent {
  args?: Input[];
  /** The node, by its id. */
  id: string;
  name: string;
}

export interface ActionWrite {
  /** The node, by its id. */
  id: string;
  /** One of its `mut` inputs. */
  input: string;
  value: Input;
}

export interface ActionSet {
  /** `$state`'s fields, by name. */
  fields: { [key: string]: Input };
}

export interface ActionExpose {
  args?: Input[];
  channel: string;
  name: string;
}

export interface ActionExtern {
  channel: string;
  /** The channel's values, by name. */
  values: { [key: string]: Input };
}

export interface ActionPage {
  /** core's page's values, by name: `width`, `theme`, `today`, `now`. */
  values: { [key: string]: Input };
}

export interface ActionRestore {
  data: Data;
}

/** One change from outside. */
export type Action =
  | { event: ActionEvent }
  | { write: ActionWrite }
  | { set: ActionSet }
  | { expose: ActionExpose }
  | { extern: ActionExtern }
  | { page: ActionPage }
  | { restore: ActionRestore };
