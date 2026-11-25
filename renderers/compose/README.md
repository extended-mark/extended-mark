# Extended Mark on Android: xtmd-android

Extended Mark pages drawn natively on Android: the IR the `xtmd` compiler writes, run by Extended Mark's native runtime (Rust, through
the Kotlin bindings UniFFI generates: [../../runtime/kotlin/rust](../../runtime/kotlin/rust)), and drawn by our default Jetpack Compose and Material 3
widgets, one composable per core extern widget.

CI builds it and runs its checks, and the release publishes
none of it. A host builds the library itself (Building, below).

```
renderers/compose/           this Gradle project, with every module below (settings.gradle.kts)
├── xtmd-android/            the renderer (xtmd:xtmd-android, as you build it): XtmdPage, XtmdDoc
├── sample/                  a host showing two pages (the shop example, and a tour of the widgets)
└── tokens.xtmdpack          core's tokens as Material 3 draws them
runtime/kotlin/              xtmd-android-runtime: the bindings and native libraries (rust/build.sh writes them)
└── rust/                    the Rust crate: xtmd-runtime behind UniFFI, build.sh, its tests
widgets/compose/             xtmd-android-widgets: the widget set, which knows no page: a widget draws a node, for a Host
live/compose/                xtmd-android-live: XtmdLiveDoc, the compiler in its own native library (rust/)
```

## Using it

A document as a host holds one is `XtmdDoc` (`XtmdDoc.kt`), Android's static document: the IR in
hand (`ir`) or in the host's assets (`src`), `$state` the host's to control or to start from, every
write checked by xtmd, and a snapshot of everything the page holds. It needs no compiler; the
live one, which compiles a document's text on the device, is xtmd-android-live.

```kotlin
var state by remember { mutableStateOf(mapOf("n" to V.num(0))) }
val doc = rememberXtmdDocState()
XtmdDoc(
    ir = pageJson,                                     // or src = "page.ir.json", in the assets
    state = state, onStateChange = { state = it },     // controlled; or defaultState = …, defaultSnapshot = …
    externs = mapOf("app" to mapOf("unit" to V.text("pts"))),
    fns = mapOf("acme/rate" to { args -> V.num(2) }),  // the host's value fns
    onError = { Log.w("app", it.toString()) },          // Refused, Failed, Load
    onCalls = { calls -> calls.forEach { handle(it.name, it.args) } },
    docState = doc,
)
val snap = doc.snapshot()?.toJson()                    // $state and every widget's own variables
doc.restore(Snapshot.fromJson(snap!!)!!)               // ALL with the same IR, STATE with another, NONE
```

- A `$state` the host gives is checked as its writes are: a field that doesn't fit is refused (`onError`) and starts
  from its default. A controlled `state` the page refuses leaves the page as it was.
- A new `ir` keeps `$state` where its fields still exist. `width`, `theme` and a channel's values in `externs` are
  followed as they change.
- `XtmdDocState` is the handle: `set`, `state`, `markdown`, `expose`, `snapshot`, `restore`, and `page`, the
  running `CompiledPage`.

The page under it:

```kotlin
// the IR `xtmd build page.xtmd --out page.ir.json` wrote, in the host's assets
val page = remember { CompiledPage.fromAsset(context, "page.ir.json", PageOptions(theme = "DARK")) }
XtmdPage(page)                                         // draws it, and keeps it drawn
XtmdPage(page, Externs(mapOf("acme/chart" to Widget { props, ctx -> MyChart(props, ctx) })))

page.set("budget" to V.num(40))                       // the host's write to $state: checked, one change
page.expose("doc/scores", "reset")                     // a fn the document exposes
for (call in page.takeCalls()) handle(call.name, call.args)   // the extern fns Extended Mark called
page.markdown()                                        // the page as Markdown, with its values now
```

- **`@Composable fun XtmdPage(page: CompiledPage, externs: Externs = Externs.None, modifier, measure, colors)`**
  draws `page.tree` with the registry's composables, each kept by its `key`, so a loop row's widgets follow its
  item. It sets the runtime's supplies from the registry, so a widget nothing draws shows its Markdown.
- **`registry`** maps every core extern widget to its composable: `"core/press" to Widget { props, ctx -> XtmdPress(props, ctx) }`.
  `Externs.widgets` replace ours by name, or add a pack's.
- **A widget** is `fun interface Widget { @Composable fun Draw(props: Props, ctx: WidgetContext) }`. `props` are
  `Val`s, worked out (`props.text("label")`, `props.slot("body")`); `ctx.event(name, args)` runs the caller's
  handler and `ctx.write(input, value)` writes a `mut` input, each as one checked change; `ctx.Children(nodes)`
  draws a slot.
- **Colours** come from the Material 3 colour scheme for the tokens a scheme has a role for (SURFACE, INK, MUTED,
  LINE, ACCENT, BAD), and from the build's values for the rest. `colors` gives the schemes, light and dark
  (Material 3's baseline by default; a host passes its own, and every page follows it), and the page's `theme`,
  else the system, picks one. tokens.xtmdpack says which values these are, and a unit test holds the two together.
- **Errors** caught by the runtime are logged with `Log.w("xtmd", …)` after each change. `XTMD_ANDROID_TRACE=1` in
  the environment logs each redraw and each spot's measured size to standard error.

### How a page is drawn

- **Every change crosses into Rust once** (`page.change { … }`), and the tree comes back in one batch when the
  revision moved.
- **Writes are exact.** A slider snaps to its grid with decimals; a pan, a zoom or a handle writes numbers rounded
  to a sub-pixel. A refused write leaves the value as it was.
- **The plane** is a clipped view holding one inner layer that `graphicsLayer` moves and scales; a custom `Layout`
  places each spot at its point by its anchor and nudge, in dp, and the background is drawn in the same layer. A
  drag pans, two fingers pinch, the wheel and `+`/`-` zoom, the arrows pan. Boxes come from the measure pass
  (`onSizeChanged`), in dp. The planes are laid out once after each change from outside the layout, and again when
  a measured size changes, never after their own layout's redraw (see "Found while building").
- **core.gfx** draws on a `Canvas` per shape, in the surface's units (y up); each shape has a box at its bounds for
  its semantics, clicks and hover. `shade` paints at most 4,096 samples from its shader, worked out in Rust off the
  main thread, and scales them up.
- **Maths** is laid out from its TeX by the same reader as @extended-mark/dom's (`Math.kt`): on a line of its own, as
  boxes on a Canvas (scripts smaller and raised or lowered, a big operator's limits above and below it, a fraction
  stacked over its bar, a root under its sign); in a sentence, as an `AnnotatedString` (scripts with a baseline
  shift, a fraction as a/b, bracketed only where it would read wrongly). TeX it doesn't know shows as itself.
- **Rows** (`flex` along X, `row`, wrapping or not) centre, start or end their children across as `align` says.
- **A closed overlay's body** stays composed, unmeasured and hidden from accessibility, as @extended-mark/dom keeps it on
  the page; open, it is a `Popup` (popover, menu, tooltip) or a `Dialog`.

### Parts

Every widget's root has its path in the semantics property `XtmdSite`, and a `testTag` with its short name; its
other parts have `testTag`s as @extended-mark/dom's `data-part`s (renderers/dom/README.md):

| Widget | Parts (`testTag`) | Semantics |
|---|---|---|
| paragraph, heading | `paragraph`, `heading` | Rich text as an `AnnotatedString`; a heading is `heading()`; a widget in a sentence is inline content drawn from the slot the runtime puts it in (`~0`, `~1` …), its room the size it measured |
| bullets, item | `bullets`; `item`, `item.check`, `item.body` | `collectionInfo`; the marker is hidden; the task box is a disabled checkbox named Done or Not done |
| quote, code, math | `quote`; `code`, `code.text`; `math` | Code scrolls sideways and is selectable; maths is named by its TeX |
| table, footnote, divider | `table`, `table.grid`, `table.header`, `table.cell`; `footnote`, `footnote.mark`, `footnote.body`; `divider` | A table is as @extended-mark/dom draws it: each column as wide as its widest cell, a line under each row, and it scrolls when wider than the page; a footnote is named "Footnote <mark>" |
| label, image, icon | `label`; `image`; `icon` | An image is named by its alt; an icon with a label is an image, else decoration |
| press | `press`, `press.label`, `press.content`, `press.indicator` | BUTTON an `OutlinedButton` (a `Button` filled with its tone when it has one), PLAIN a `TextButton`, TOGGLE and OPTION a tonal or outlined button with `selected`, TAB a `Tab`, SWITCH, CHECK and RADIO a row with role switch, checkbox or radio button; with content, its label is its name |
| field | `field`, `field.label`, `field.input` | An `OutlinedTextField`; Enter (its IME action) submits a one-line field, Ctrl+Enter a longer one |
| slider | `slider`, `slider.label`, `slider.input`, `slider.value` | A Material `Slider` named by its label; arrows, Page Up/Down, Home and End move it exactly |
| overlay | `overlay`, `overlay.anchor`, `overlay.body` | The anchor is a button unless it holds a control; the body is a pane named by its label; Escape closes it |
| canvas, layer | `canvas`, `canvas.surface`; `layer`, `layer.shapes` | A canvas is named by its label; its surface clicks when its click is set |
| circle, path, place | `circle`; `path`, `path.line`, `path.head`; `place`, `place.box` | A shape with a click is a button, one with a label an image |
| handle, shade | `handle`, `handle.dot`; `shade`, `shade.pixels` | A handle is focusable, with its point as its state; arrows move it 8 dp, 40 with Shift |
| flex, grid, cell, sizing | `flex`, `grid`, `cell`, `sizing` | Clickable when their click does something; a run of tabs, radios or options is a selectable group |
| plane, spot | `plane`, `plane.view`, `plane.inner`, `plane.background`, `plane.items`; `spot` | The view is focusable and takes the keys |
| a widget that failed | `notice` | A polite live region |

## Building

What this box has, and the project expects: Java 21, the Android SDK at `~/.local/android` (`local.properties`;
platform 36, build-tools 36.1.0 and NDK 30.0.16248370, all pinned in the build files so AGP fetches nothing),
Rust with the Android targets. Gradle comes from the wrapper (8.14.4); AGP 8.13.2, Kotlin 2.2.21, Compose BOM
2025.10.01 and Robolectric 4.16.1, a combination known to work together.

```sh
runtime/kotlin/rust/build.sh            # 1. libxtmd_android.so for the host, arm64-v8a, armeabi-v7a and x86_64, and the bindings
cd renderers/compose
./gradlew assembleRelease              # 2. the AARs: <module>/build/outputs/aar/, of the three modules
./gradlew publish                      #    or published, as xtmd:xtmd-android:0.4.0 and what it needs, to build/repo
./gradlew :sample:assembleDebug         # 3. the APK: sample/build/outputs/apk/debug/sample-debug.apk
./gradlew testDebugUnitTest               # the unit tests (Robolectric)
```

The library's build stops with a message when `native/` is missing (step 1). The sample builds its pages' IR with
`compiler/target/release/xtmd` (`cargo build --release` in compiler) as part of its build:
`examples/pages/shop.xtmd` and `sample/pages/tour.xtmd`. The APK carries arm64-v8a, armeabi-v7a (budget phones often run a 32-bit-only Android) and x86_64; build.sh needs `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`.

## Keeping it fast

A budget phone is 5 to 10 times slower than a desktop, and the runtime redraws the whole page on
every change. So a change's work is kept small, and a drag's is kept off the frames:

- **A redraw sends only what changed.** `NativePage.treeUpdate(since)` sends each widget that is as
  it was (and everything in it) as a stub, and `CompiledPage` keeps the very node it holds for it,
  so only changed nodes cross from Rust, and Compose (with strong skipping) composes only those:
  about 6 widgets a step of a drag on the tour, not all 52.
- **A drag writes at most once a frame, and off the main thread.** `FrameWriter` keeps the latest
  move, writes it on the next frame through `CompiledPage.writeAway` (the runtime's work on a
  background thread, the tree read back on the main one), and never has two writes in flight. The
  handle, the slider's thumb and the plane follow the finger meanwhile; where a drag ends is always
  written. Keys and accessibility actions write at once, as before.
- **The planes are laid out only when a plane or a spot (or anything in one) changed**, or a spot's
  measured size did; a layout that moved a box lays out again on the next change, as one that
  doesn't settle needs (the checks hold this to the tests' behaviour).
- **One colour scheme for as long as the mode lasts**: a new one at each redraw was a new value of
  MaterialTheme's static composition local, and composed the whole page again.
- **Pages load off the main thread**: `CompiledPage.loadAsset` (or `rememberAssetPage` in a
  composable) parses the IR and mounts the page on a background thread; an IR with all of core in
  it takes 40 to 120 ms to mount here, a fraction of a second on a phone. The sample shows a
  progress indicator meanwhile.
- **Tests wait for the off-main work**: `XtmdWork.idle` is false while a load, a drag's write or a
  pending frame write runs; every test registers it as an idling resource.

`DragPerfTest` measures the tour's three drags (the handle, the plane, the slider) on a phone-sized
window, each step's main-thread processor time split from the runtime's off-main time, against a
control step, and holds them to a budget and to what the fixes do (only changed widgets composed
and sent, one write a step). It writes `xtmd-android/build/drag-perf.txt`. `-Pxtmd.jfr=<file>` records
a Java Flight Recording of the tests, and `-Pxtmd.perf.only=handle|plane|slider` runs one drag.

## Looking at it

`./gradlew :sample:testDebugUnitTest` draws the sample in `sample/build/render` (or where `-Pxtmd.render=<folder>`
says) under Robolectric's native graphics (Skia, real fonts), as an Infinix X6516 shows it (360 by 806 dp at density 2), light and dark: each tab's first
screen with the host around it, and each page whole. The same pages drawn by @extended-mark/dom in Chromium at 360 px wide
are what to hold them against. Compose's `captureToImage()` waits for a frame Robolectric's paused looper never
draws, so `RenderTest` draws the window's views on a canvas itself.

## Found while building (what took trial and error)

- **JNA**: the host takes `net.java.dev.jna:jna:5.17.0@aar` (libjnidispatch for each processor); the unit tests take
  the plain jar too (the host's libjnidispatch), and find libxtmd_android.so through `-Djna.library.path`, set
  on the Gradle test task.
- **The plane's layout isn't idempotent for a spot with no box variable** (the graphs pack's link labels): each
  `layout()` writes its own `box` again and redraws. Laying out after every redraw never settled (Compose "did not
  get idle"); laying out once per change from outside the layout (`changes`, as the tests do once per step)
  does. With Compose's own measure the first layout waits for the spots' first measure, so the first boxes written
  are the real ones.
- **Hover under Robolectric:** a shape with both a `hoverable` and a `clickable`, on two interaction sources,
  never got its Exit; one source shared by both does.
- **Space and Enter** activate Compose's buttons from a hardware keyboard, as on the web; a focus on a
  disabled control does nothing (it has no focus action), as on the web.
- **Robolectric's first run downloads its Android 16 jar** (about 200 MB into ~/.m2); later starts take about 10 s.
- **The compiler drops prose `@slot name {` blocks** (an overlay in prose gets an empty anchor and no body), so the
  tour page declares its overlay and its plane as widgets of its own.

## Gaps

- Pictures inside rich text show their alt text; `image` loads http, https and data: pictures itself, with no cache
  beyond the composition.
- Maths in a sentence is one line of text: a fraction there is a/b, not stacked; on a line of its own it is laid
  out, but without TeX's fine spacing rules or a maths font (it uses the system serif).
