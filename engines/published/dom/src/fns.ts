// The value fns the web supplies: the reader's own time zone, which only the browser knows.
// can_draw_tex keeps core's default: TeX our reader doesn't know is shown as itself.

import type { Input } from "@extended-mark/runtime";
import { field, flag, num } from "./values.js";

const int = (n: number): Input => ({ num: String(n) });

/** A record like `like`, with some of its fields changed. */
function withFields(like: Input & { record: unknown }, set: Record<string, Input>): Input {
  const r = (like as Extract<Input, { record: unknown }>).record;
  return { record: { type: r.type, fields: { ...r.fields, ...set } } };
}

/**
 * `core/moment.local`: the same instant, with the offset the reader's zone uses on its date,
 * summer time included. Worked out from the moment's fields, as the page may call it while it
 * is still being made.
 */
export function webFns(): Record<string, (args: readonly Input[]) => Input | null> {
  return {
    "core/moment.local": ([m]) => {
      const day = field(m, "day");
      if (!m || !("record" in m) || !day || !("record" in day)) return null;
      const n = (v: Input | undefined) => num(v) ?? 0;
      const sign = flag(field(m, "east")) ? 1 : -1;
      const offset = sign * (n(field(m, "offset_hour")) * 60 + n(field(m, "offset_minute")));
      // the instant, in UTC; setUTCFullYear keeps years below 100 as they are
      const at = new Date(0);
      at.setUTCFullYear(n(field(day, "year")), n(field(day, "month")) - 1, n(field(day, "day")));
      at.setUTCHours(n(field(m, "hour")), n(field(m, "minute")) - offset, 0, 0);
      const east = -at.getTimezoneOffset();
      const local = new Date(at.getTime() + east * 60_000);
      return withFields(m, {
        day: withFields(day, { year: int(local.getUTCFullYear()), month: int(local.getUTCMonth() + 1), day: int(local.getUTCDate()) }),
        hour: int(local.getUTCHours()),
        minute: int(local.getUTCMinutes()),
        east: { bool: east >= 0 },
        offset_hour: int(Math.floor(Math.abs(east) / 60)),
        offset_minute: int(Math.abs(east) % 60),
      });
    },
  };
}
