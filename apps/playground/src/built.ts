// What the host adds to the page built ahead (/built): the reader's theme, told to the page as it changes.

import type { MountedPage } from "@extended-mark/web/kit";

const built = document.querySelector<HTMLScriptElement>("script[data-page]")!;
const { page } = (await import(built.src)) as { page?: MountedPage };
const dark = matchMedia("(prefers-color-scheme: dark)");
const tell = () => page?.act({ page: { values: { theme: { case: { enum: "core/theme", name: dark.matches ? "DARK" : "LIGHT" } } } } });
dark.addEventListener("change", tell);
tell();
