// What the playground, as the host, gives a page: where it is read (width and theme), and the clock
// (`core/page.today` and `core/page.now`).

import type { Input, PageValues } from "@extended-mark/runtime";

const n = (v: number): Input => ({ num: String(v) });

/** The page's width, from the width it is drawn at, in CSS pixels. */
export function widthOf(px: number): NonNullable<PageValues["width"]> {
  if (px < 600) return "NARROW";
  if (px < 1000) return "MEDIUM";
  return "WIDE";
}

export function themeNow(): NonNullable<PageValues["theme"]> {
  return matchMedia("(prefers-color-scheme: dark)").matches ? "DARK" : "LIGHT";
}

function offsetFields(minutesEast: number) {
  const abs = Math.abs(minutesEast);
  return { east: minutesEast >= 0, hour: Math.min(14, Math.floor(abs / 60)), minute: abs % 60 };
}

/** `core/page.today` and `core/page.now`, from the browser's clock and zone. */
export function clockExterns(now = new Date()): Record<string, Record<string, Input>> {
  const today: Input = { record: { type: "core/date", fields: { year: n(now.getFullYear()), month: n(now.getMonth() + 1), day: n(now.getDate()) } } };
  const off = offsetFields(-now.getTimezoneOffset());
  const moment: Input = {
    record: {
      type: "core/moment",
      fields: { day: today, hour: n(now.getHours()), minute: n(now.getMinutes()), east: { bool: off.east }, offset_hour: n(off.hour), offset_minute: n(off.minute) },
    },
  };
  return { "core/page": { today, now: moment } };
}
