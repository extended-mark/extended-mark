// Extended Mark's Android engines: `xtmd-android-runtime` (the native runtime and its Kotlin bindings),
// `xtmd-android-widgets` (the Compose widget set), `xtmd-android` (the renderer: XtmdPage and XtmdDoc),
// `xtmd-android-live` (the live one: XtmdLiveDoc, with the compiler in its own native library), and
// `sample`, a host showing a page. The unit tests run Compose under Robolectric on the JVM.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = "xtmd-android-target"
include(":xtmd-android-runtime", ":xtmd-android-widgets", ":xtmd-android", ":xtmd-android-live", ":sample")
