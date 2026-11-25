# xtmd-android-live

Extended Mark's live documents on Android: `XtmdLiveDoc` compiles a document's text on the device as it is
edited, and draws it with [xtmd-android](../../renderers/compose/xtmd-android)'s `XtmdDoc`. The compiler is in its own
native library, `libxtmd_android_live.so` ([./rust](./rust): xtmd's compiler behind
UniFFI, `xtmd.android.live.ffi`). It hands Kotlin the IR as JSON, which `XtmdDoc` runs on
xtmd-android's own library, so nothing crosses between the two. A host that only draws pages it
was given uses xtmd-android alone, and ships no compiler.

```kotlin
var text by remember { mutableStateOf(initialText) }        // the editor's
XtmdLiveDoc(
    source = text,                                         // when it changes is the host's: each key, a pause, a save
    packs = listOf(PackFile("graphs/graphs.xtmdpack", graphsText)),
    onError = { e -> if (e is DocError.Compile) showProblem(e.message) },   // the first error and where
    onDiagnostics = { list -> markers = list },            // every diagnostic of the latest text
    // and everything XtmdDoc takes: state, onStateChange, externs, fns, docState …
)
```

- A text that compiles replaces the page, keeping `$state` where its fields still exist. One that
  doesn't leaves the last page that did.
- The compile runs off the main thread (`LiveCompiler.build`). Core is compiled the first time,
  once for the host. A newer text cancels the answer of the one before.
- `LiveCompiler.get()` is the host's compiler, to build a text outside a composable.

## Building and testing

```sh
../../runtime/kotlin/rust/build.sh host && ./rust/build.sh host   # both host libraries and their bindings
./rust/build.sh android                           # the compiler for arm64-v8a, armeabi-v7a and x86_64
./gradlew :xtmd-android-live:testDebugUnitTest             # XtmdLiveDocTest, under Robolectric
```

