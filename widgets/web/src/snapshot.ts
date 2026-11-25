// What every document shares of a snapshot, and of what it tells the host went wrong.

import type { Input } from "@extended-mark/runtime";

/** What went wrong, for `onError`. */
export type DocError =
  | { kind: "refused"; rule?: string; code?: string; site?: string }
  | { kind: "failed"; rule?: string; code?: string; site?: string }
  /** A live document's compiler couldn't be loaded. */
  | { kind: "load"; message: string }
  | { kind: "compile"; message: string; diagnostics: unknown[] };

/** Everything the page holds, as JSON: opaque to the host, which keeps it and gives it back. */
export interface Snapshot {
  xtmd: string;
  /** A hash of the IR it was taken from: its places mean the same widgets only in the same IR. */
  ir: string;
  state: Record<string, Input>;
  own: Record<string, Record<string, Input>>;
}

/** What `restore` did. */
export interface Restored {
  /** "all" when the IR is the one it was taken from; "state" when only `$state` could go back;
   * "none" when the snapshot can't be read or its `$state` was refused. */
  restored: "all" | "state" | "none";
  /** What was left out: refused fields and places, or "own" for every widget's variables. */
  dropped: string[];
  why?: string;
}

/** The data a page of this IR starts from, from a snapshot: all of it for the IR it was taken from, `$state`
 * only for another, and none from one this release can't read. */
export function dataOf(snap: Snapshot | undefined, ir: string): { state?: Record<string, Input>; own?: Record<string, Record<string, Input>> } | undefined {
  if (!snap || typeof snap !== "object" || snap.xtmd !== RELEASE || typeof snap.state !== "object") return undefined;
  return snap.ir === ir ? { state: snap.state, own: snap.own ?? {} } : { state: snap.state };
}

/** A short, stable hash of a text: FNV-1a, 52 bits, in base 36. */
export function hash(text: string): string {
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  for (let k = 0; k < text.length; k++) {
    const c = text.charCodeAt(k);
    h1 = Math.imul(h1 ^ c, 16777619) >>> 0;
    h2 = Math.imul(h2 ^ c, 2246822519) >>> 0;
  }
  return (h2 % 0x100000).toString(36) + h1.toString(36);
}

/** The release snapshots are taken in: one of another can't be read. The package's version, written by
 * tools/release/bump.mjs and held to it by `bump.mjs --check`. */
export const RELEASE = "0.4.0";
