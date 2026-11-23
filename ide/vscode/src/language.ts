// Which language a file is read in. A pack (`.xtmdpack`) is definitions only (`xtmd-pack`). A document
// (`.xtmd`) is code then prose when it has a `@@@` line (`xtmd`); with none, prose all through
// (`xtmd-prose`), unless it is only code. A TextMate grammar reads line by line and can't look ahead
// for the line, so the extension picks, as the playground does.

const CODE_FIRST = /^\s*(?:package|import|\$state|widget|record|enum|fn|let|extern|expose|token|tokens|type|interface|pack)\b/;

export function languageFor(path: string, text: string): "xtmd" | "xtmd-prose" | "xtmd-pack" {
  if (path.endsWith(".xtmdpack")) return "xtmd-pack";
  if (!path.endsWith(".xtmd")) return "xtmd";
  if (hasDivider(text)) return "xtmd";
  let lines = text.split("\n");
  // front matter is neither
  if (lines[0]?.trim() === "---") {
    const end = lines.findIndex((l, k) => k > 0 && l.trim() === "---");
    if (end > 0) lines = lines.slice(end + 1);
  }
  const first = lines.find((l) => l.trim() !== "" && !l.trim().startsWith("//"));
  return first !== undefined && CODE_FIRST.test(first) ? "xtmd" : "xtmd-prose";
}

/** Whether a document has its `@@@` line, outside any fenced code block that is closed. */
function hasDivider(text: string): boolean {
  const lines = text.split("\n");
  for (let k = 0; k < lines.length; k++) {
    if (/^ {0,3}@@@\s*$/.test(lines[k])) return true;
    const open = /^ {0,3}(`{3,}(?=[^`]*$)|~{3,})/.exec(lines[k]);
    if (!open) continue;
    const closes = new RegExp(`^ {0,3}${open[1][0]}{${open[1].length},}\\s*$`);
    const end = lines.findIndex((l, j) => j > k && closes.test(l));
    if (end > 0) k = end;
  }
  return false;
}
