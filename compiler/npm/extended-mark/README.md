# extended-mark

Extended Mark's compiler: the `xtmd` command, and its API and Vite plugin for build tools. (npm
doesn't allow the name `xtmd` itself, so the package has the language's name; the command is `xtmd`.)

```sh
npm i -g extended-mark       # the command, a prebuilt binary for this machine
npx extended-mark check .    # or without installing
npm i -D extended-mark       # the API and the Vite plugin, in a project
```

Without Node: `cargo install xtmd`, or the one-line installers in the
[repository's README](https://github.com/extended-mark/extended-mark#install).

As an API, every call runs the native `xtmd` binary: there is one compiler
(compiler/, in Rust), and nothing here turns source into IR itself. On a platform with no binary,
`build` and `check` run the same compiler built as WebAssembly (@extended-mark/wasm), so they work wherever
Node does.

```js
import { build, check, md, format } from "extended-mark";

const page = await build("sprint.xtmd", { packs: ["./packs/maps"] }); // { ir: … }, lowered
const diagnostics = await check("sprint.xtmd");                       // [] when it compiles
const markdown = await md("sprint.xtmd");                              // with its starting values
```

With Vite, `import page from "./sprint.xtmd"` gives the page's IR:

```js
// vite.config.js
import xtmd from "extended-mark/vite";
export default { plugins: [xtmd({ packs: ["./packs/maps"] })] };
```

`npx extended-mark check|fmt|compile|decompile|export|md|surface|migrate …` runs the binary with the same arguments: `npx extended-mark compile doc.xtmd --rir` writes its resolved IR for tools, `npx extended-mark decompile doc.rir.json` writes it back as source, `npx extended-mark surface packs/kit` writes a released version's surface, and `npx extended-mark migrate docs/` brings documents up to the versions of the packs they are built with (`fmt`, `surface` and `migrate` each take `--check` to fail if anything would change, `--diff` to show the change and `--stdout` to print one file's result, and write in place otherwise; migrate asks first in a terminal; `--yes` writes elsewhere, `--yes all` also answers every numbered question yes and `--yes 1,3` those only, while a bare `--yes` with questions open lists them and writes nothing; `--dry-run` only looks; `--allow-todos` writes what it can't finish with a TODO, `--allow-errors` writes a file that doesn't compile; `--format json` prints a JSON object for each file, note and question, and `--report run.md` or `run.json` writes the whole run). An `xtmd.toml` beside the tree says what it is written for, `xtmd = "0.4.0"` and `[uses]` with each pack's version, and `xtmd migrate` moves both with it. `npx extended-mark migrate docs/` moves a tree written for the release before this one by what each release understands of it; one further back is moved by each release in turn (docs/guides/migration.md in the repository).

## The binary

`xtmd` finds it, in order: `XTMD_BINARY`; the package for this platform,
`@extended-mark/cli-<os>-<cpu>`, an optional dependency; inside this repository, `compiler/target`'s
release or debug build. With none of them, `build` and `check` compile with @extended-mark/wasm, which
gives the same IR and the same diagnostics; it reads no `xtmd.toml`, so a document whose packs'
versions one names needs the binary, and `md`, `format` and the `xtmd` command need it too.

There is a package for Linux on x64 and arm64, macOS on Intel and Apple silicon, and Windows on
x64. The release builds every one on Linux with cargo-zigbuild (.github/workflows/release.yml);
by hand:

```sh
cd compiler && cargo build --release -p xtmd
cd npm/extended-mark && npm run binary                   # this machine's, to platforms/<os>-<cpu>/bin
cd ../.. && cargo zigbuild --release -p xtmd --target aarch64-apple-darwin
cd npm/extended-mark && npm run binary -- --target aarch64-apple-darwin   # to platforms/darwin-arm64/bin
```

`npm test` runs the JS API against the binary and builds `test/vite-app` with Vite.
