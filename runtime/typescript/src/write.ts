// Markdown blocks as text: one spelling for each block and mark, and a character in a value that Markdown
// would read is written with a backslash (page 1, Markdown out). It knows no page: whoever has blocks
// (`schemas/blocks.schema.json`) may write them. compiler/xtmd/src/markdown writes the same text.

import type { Block, BlockBulletsItems, Inline } from "./generated/blocks.js";

export type { Block, Inline };
/** An item of a list. */
export type Item = BlockBulletsItems;

function escape(s: string, lineStart: boolean): string {
  let out = "";
  let atStart = lineStart;
  const chars = [...s];
  chars.forEach((c, k) => {
    const before = chars.slice(0, k).join("");
    let esc = "\\`*_~[]<>$|&".includes(c) || (atStart && "#-+=@}!".includes(c));
    if (c === ":" && /(https?|ftp)$/.test(before)) esc = true;
    if (c === "." && /www$/i.test(before)) esc = true;
    if ((c === "." || c === ")") && lineStart && before.length > 0 && /^[0-9]+$/.test(before)) esc = true;
    if (esc) out += "\\";
    out += c;
    if (c === "\n") atStart = true;
    else if (!(c === " " && atStart)) atStart = false;
  });
  return out;
}

function codeSpan(c: string): string {
  let ticks = 1;
  while (c.includes("`".repeat(ticks))) ticks++;
  const fence = "`".repeat(ticks);
  const pad = c.startsWith("`") || c.endsWith("`") || (c.startsWith(" ") && c.endsWith(" ") && c.trim() !== "") ? " " : "";
  return fence + pad + c + pad + fence;
}

type Mark = "link" | "strike" | "bold" | "italic";

/** Whether two runs of text have the mark, the same one. */
function shares(m: Mark, a: Inline, b: Inline): boolean {
  return "text" in a && "text" in b && a.text[m] !== undefined && a.text[m] !== false && a.text[m] === b.text[m];
}

/** A sentence's runs, each mark written once around all the runs that share it. */
export function writeInline(runs: Inline[]): string {
  return writeRuns(runs, ["link", "strike", "bold", "italic"], true);
}

function writeRuns(runs: Inline[], marks: Mark[], lineStart: boolean): string {
  let out = "";
  for (let k = 0; k < runs.length; ) {
    const r = runs[k];
    const first = lineStart && out === "";
    // the mark that goes around the most runs from here; a link goes around any other
    let mark: Mark | undefined;
    let most = 0;
    for (const m of marks) {
      let n = 0;
      while (k + n < runs.length && shares(m, r, runs[k + n])) n++;
      if (n > 0 && mark !== "link" && n > most) [mark, most] = [m, n];
    }
    if (mark !== undefined && "text" in r) {
      const inner = writeRuns(runs.slice(k, k + most), marks.filter((m) => m !== mark), first);
      k += most;
      if (mark === "link") {
        out += `[${inner}](<${r.text.link!.replace(/</g, "%3C").replace(/>/g, "%3E")}>)`;
        continue;
      }
      const d = mark === "strike" ? "~~" : mark === "bold" ? "**" : "*";
      // a mark with a space just inside it doesn't read as one, so the space goes outside
      const start = /^[ \n\t]*/.exec(inner)![0].length;
      const end = inner.length - /[ \n\t]*$/.exec(inner)![0].length;
      out += end <= start ? inner : inner.slice(0, start) + d + inner.slice(start, end) + d + inner.slice(end);
      continue;
    }
    if ("text" in r) out += escape(r.text.value, first);
    else if ("code" in r) out += codeSpan(r.code);
    else if ("math" in r) out += `$${r.math}$`;
    else if ("image" in r) out += `![${escape(r.image.alt, false)}](<${r.image.src}>)`;
    else if ("note" in r) out += `[^${r.note}]`;
    else out += "\\\n";
    k++;
  }
  return out;
}

function indent(text: string, first: string, rest: string): string {
  return (
    text
      .split("\n")
      .filter((_, k, all) => k < all.length - 1 || all[k] !== "")
      .map((line, k) => {
        const prefix = k === 0 ? first : rest;
        return line === "" ? prefix.trimEnd() : prefix + line;
      })
      .join("\n") + "\n"
  );
}

function writeBlock(b: Block): string {
  if ("paragraph" in b) return writeInline(b.paragraph) + "\n";
  if ("heading" in b) return `${"#".repeat(b.heading.level)} ${writeInline(b.heading.inline)}\n`;
  if ("bullets" in b) {
    const { ordered, items } = b.bullets;
    const start = b.bullets.start ?? 1;
    const loose = items.some((i) => i.blocks.length > 1);
    return items
      .map((it, k) => {
        const marker = ordered ? `${start + k}. ` : "- ";
        const task = it.done === true ? "[x] " : it.done === false ? "[ ] " : "";
        const body = writeBlocks(it.blocks) || "\n";
        return indent(body, marker + task, " ".repeat(marker.length)) + (loose && k + 1 < items.length ? "\n" : "");
      })
      .join("");
  }
  if ("quote" in b) return indent(writeBlocks(b.quote), "> ", "> ");
  if ("code" in b) {
    let ticks = 3;
    while (b.code.src.includes("`".repeat(ticks))) ticks++;
    const fence = "`".repeat(ticks);
    const src = b.code.src.endsWith("\n") || b.code.src === "" ? b.code.src : b.code.src + "\n";
    return `${fence}${b.code.lang}\n${src}${fence}\n`;
  }
  if ("math" in b) return `$$\n${b.math.trimEnd()}\n$$\n`;
  if ("divider" in b) return "---\n";
  if ("table" in b) {
    const cell = (c: Inline[]) => writeInline(c).replace(/\|/g, "\\|");
    const { head, rows, align } = b.table;
    const rule = head.map((_, k) => (align[k] === "CENTER" ? ":---:" : align[k] === "END" ? "---:" : "---"));
    return [`| ${head.map(cell).join(" | ")} |`, `| ${rule.join(" | ")} |`, ...rows.map((r) => `| ${r.map(cell).join(" | ")} |`)].join("\n") + "\n";
  }
  return indent(writeBlocks(b.footnote.blocks), `[^${b.footnote.mark}]: `, "    ");
}

function writeBlocks(blocks: Block[]): string {
  return blocks.map(writeBlock).join("\n");
}

/** The page as Markdown text: its blocks, then its footnotes at the end. */
export function writeMarkdown(blocks: Block[]): string {
  const notes = blocks.filter((b) => "footnote" in b);
  const rest = blocks.filter((b) => !("footnote" in b));
  return [...rest, ...notes].map(writeBlock).join("\n");
}
