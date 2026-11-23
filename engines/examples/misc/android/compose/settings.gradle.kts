// Extended Mark's Android engines: `xtmd-android` (the static library: the Kotlin bindings to the native
// runtime, our default Compose widgets, XtmdPage and XtmdDoc), `xtmd-android-live` (the live one:
// XtmdLiveDoc, with the compiler in its own native library), and `sample`, a host showing a page. xtmd-android's
// unit tests run Compose under Robolectric on the JVM.
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
include(":xtmd-android", ":xtmd-android-live", ":sample")
