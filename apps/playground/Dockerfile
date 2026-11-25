# syntax=docker/dockerfile:1
#
# The Extended Mark playground: its Node server, and the page, which compiles in the browser with xtmd.wasm.
# Build it with the repository root as the context:
#
#   docker build -f apps/playground/Dockerfile -t xtmd-playground .
#   docker run --rm -p 8080:8080 -e DEVBENCH_NOTIFY_URL=… -e DEVBENCH_NOTIFY_TOKEN=… xtmd-playground
#
# Three stages: the compiler as WebAssembly, the UI's bundle, and what runs. What runs is Node,
# server.mjs, the bundle (xtmd.wasm in it), the examples and the packs: no node_modules, as the
# server uses Node's own modules only, and no compiler binary.

# ---- 1. the compiler: xtmd.wasm, for wasm32-wasip1, with core embedded in it at build time;
#         decNumber's C is compiled by zig's clang (compiler/wasm/zig/cc)

FROM rust:1-bookworm AS compiler
RUN apt-get update && apt-get install -y --no-install-recommends python3-venv && rm -rf /var/lib/apt/lists/* \
 && python3 -m venv /opt/zig && /opt/zig/bin/pip install --no-cache-dir ziglang==0.17.1 \
 && rustup target add wasm32-wasip1
ENV XTMD_ZIG="/opt/zig/bin/python -m ziglang"
WORKDIR /src
# the live crate embeds ../../core, and the syntax crate includes core/entities.xtmdpack
COPY core ./core
COPY compiler ./compiler
# the other members of compiler/'s workspace, so cargo can load it
COPY runtime ./runtime
COPY widgets/egui ./widgets/egui
COPY renderers/egui ./renderers/egui
COPY live/egui ./live/egui
COPY live/compose/rust ./live/compose/rust
RUN compiler/wasm/build.sh /src/xtmd.wasm && test -s /src/xtmd.wasm

# ---- 2. the UI: what it is bundled from built (the runtime, the widgets, the renderers, the engine, the compiler's
#         package and the live documents), then bundled
FROM node:22-bookworm-slim AS ui
ENV npm_config_fund=false npm_config_audit=false npm_config_update_notifier=false
WORKDIR /src
COPY runtime ./runtime
COPY widgets ./widgets
COPY renderers ./renderers
COPY engines ./engines
COPY live ./live
COPY compiler/npm/wasm ./compiler/npm/wasm
COPY --from=compiler /src/xtmd.wasm ./compiler/npm/wasm/xtmd.wasm
COPY apps/playground ./apps/playground
# each package installed, then built, in the order they need each other: npm copies a file:
# package (or links it) as it is then, so what it needs is built first. The packages' tests
# need the xtmd binary (extended-mark), which isn't built here
RUN set -e; \
    for d in compiler/npm/wasm runtime/typescript widgets/web renderers/dom engines/web live/web renderers/react; do \
      cd "/src/$d"; \
      npm pkg delete devDependencies.extended-mark; \
      rm -rf node_modules dist package-lock.json; \
      npm install; \
      npm run build; \
    done; \
    # a page compiled with xtmd.wasm: a broken one stops the build here
    cd /src/compiler/npm/wasm && node -e "import('./dist/index.js').then(async ({ Compiler }) => { const b = (await Compiler.load()).build('page.xtmd', '# Hello\\n'); if (b.errors || !b.ir) throw new Error(JSON.stringify(b.diagnostics)); })"; \
    cd /src/apps/playground && rm -rf node_modules dist && npm ci && npm run build \
 && test -s dist/app.js && test -s dist/app.css && test -s dist/editor.worker.js && test -s dist/compiler.worker.js && test -s dist/xtmd.wasm \
 && test -s dist/built.html && test -s dist/embedded.html && test -s dist/live.html && test -s dist/basket.ir.json && ls dist/codicon-*.ttf

# ---- 3. what runs

FROM node:22-bookworm-slim
ENV NODE_ENV=production \
    PORT=8080
WORKDIR /app
# the packs a page may import (server.mjs: PACKS), where they are in the repo
COPY examples/graphs ./examples/graphs
COPY --from=ui /src/apps/playground/server.mjs /src/apps/playground/index.html /src/apps/playground/package.json ./apps/playground/
COPY --from=ui /src/apps/playground/examples ./apps/playground/examples
COPY --from=ui /src/apps/playground/dist ./apps/playground/dist
USER node
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["node", "-e", "fetch('http://127.0.0.1:' + (process.env.PORT || 8080) + '/healthz').then((r) => process.exit(r.ok ? 0 : 1), () => process.exit(1))"]
CMD ["node", "apps/playground/server.mjs"]
