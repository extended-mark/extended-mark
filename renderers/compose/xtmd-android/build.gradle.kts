import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

// xtmd-android: Extended Mark's Compose renderer, XtmdPage and XtmdDoc: the native runtime
// (xtmd-android-runtime) drawn with the Compose widget set (xtmd-android-widgets).
plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    `maven-publish`
}

// the widget set copied into a package of a host's own, as a host would start from it (CopiedSetTest)
val copiedSet = tasks.register<Sync>("copyWidgetSet") {
    from(project(":xtmd-android-widgets").layout.projectDirectory.dir("src/main/kotlin/xtmd/android/widgets"))
    into(layout.buildDirectory.dir("copied/copied"))
    filter { it.replace(Regex("^package xtmd\\.android\\.widgets$"), "package copied") }
}

android {
    namespace = "xtmd.android"
    compileSdk = 36
    // the build tools and NDK this box has (README): AGP would otherwise fetch its own defaults
    buildToolsVersion = "36.1.0"
    ndkVersion = "30.0.16248370"
    defaultConfig { minSdk = 26 }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true }
    sourceSets {
        getByName("test") {
            // the check packs' Android widgets (checks/packs/*/android.kt), as a host adds a pack's
            java.srcDir(rootProject.file("../../checks/packs/badge"))
            java.srcDir(layout.buildDirectory.dir("copied"))
        }
    }
    testOptions { unitTests.isIncludeAndroidResources = true }
    lint { abortOnError = false }
    publishing { singleVariant("release") { withSourcesJar() } }
}

// Published as xtmd:xtmd-android: `./gradlew :xtmd-android:publish` writes it to ../build/repo, a
// Maven repository a host's build can name (README.md: the AAR).
afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "xtmd"
                artifactId = "xtmd-android"
                version = "0.4.0"
            }
        }
        repositories { maven { url = uri(rootProject.layout.buildDirectory.dir("repo")) } }
    }
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach { dependsOn(copiedSet) }

kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }

dependencies {
    api(project(":xtmd-android-runtime"))
    api(project(":xtmd-android-widgets"))

    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.robolectric)
    testImplementation(libs.junit)
    // the plain jar: libjnidispatch for the host, which the AAR doesn't carry
    testImplementation(libs.jna)
    // ComponentActivity in the test manifest, for runAndroidComposeUiTest
    debugImplementation(libs.compose.ui.test.manifest)
}

// The unit tests load the host's libxtmd_android.so through JNA, and the
// live engine's libxtmd_android_live.so.
tasks.withType<Test>().configureEach {
    val runtimeHost = project(":xtmd-android-runtime").layout.projectDirectory.dir("native/host/linux-x86-64").asFile
    val liveHost = project(":xtmd-android-live").layout.projectDirectory.dir("native/host/linux-x86-64").asFile
    systemProperty("jna.library.path", listOf(runtimeHost, liveHost).joinToString(File.pathSeparator) { it.absolutePath })
    maxHeapSize = "2g"
    // `-Pxtmd.jfr=<file>`: a Java Flight Recording of the tests, to see where their time goes
    System.getenv("XTMD_ANDROID_TRACE")?.let { environment("XTMD_ANDROID_TRACE", it) }
    (project.findProperty("xtmd.perf.only") as String?)?.let { systemProperty("xtmd.perf.only", it) }
    (project.findProperty("xtmd.jfr") as String?)?.let { jvmArgs("-XX:StartFlightRecording=filename=$it,settings=profile,jdk.ExecutionSample#period=1ms") }
}
