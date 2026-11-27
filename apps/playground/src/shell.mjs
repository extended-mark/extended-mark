// The HTML around the playground's three pages (/built, /embedded, /live): its bar and what the page is of. build.mjs
// writes each into dist/.

const NAV = [
  ["/", "Playground"],
  ["/built", "Built ahead"],
  ["/embedded", "Embedded"],
  ["/live", "Live"],
];

/** A page: its path, its title, what it shows in a sentence, and its HTML. */
export function shell({ path, title, about, main, icon, styles = [], scripts = [], head = "" }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>${title} · Extended Mark</title>
<meta name="description" content="${about}">
${icon}
<link rel="stylesheet" href="/pages.css">
${styles.map((s) => `<link rel="stylesheet" href="${s}">`).join("\n")}
${scripts.map((s) => `<script type="module" src="${s}"></script>`).join("\n")}
${head}</head>
<body>
<header class="bar">
<a class="name" href="/">Extended Mark</a>
<nav aria-label="Pages">
${NAV.map(([to, words]) => `<a href="${to}"${to === path ? ' aria-current="page"' : ""}>${words}</a>`).join("\n")}
</nav>
</header>
${main}
</body>
</html>
`;
}
