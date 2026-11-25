import org.jetbrains.kotlin.gradle.dsl.JvmTarget

// xtmd-android-widgets: Extended Mark's Compose widget set, a Material 3 composable for every core extern
// widget. A widget knows no page: it draws a node, and acts through a Host.
plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    `maven-publish`
}

android {
    namespace = "xtmd.android.widgets"
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
    testOptions { unitTests.isIncludeAndroidResources = true }
    lint { abortOnError = false }
    publishing { singleVariant("release") { withSourcesJar() } }
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "xtmd"
                artifactId = "xtmd-android-widgets"
                version = "0.4.0"
            }
        }
        repositories { maven { url = uri(rootProject.layout.buildDirectory.dir("repo")) } }
    }
}

kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }

dependencies {
    // a page's nodes and values
    api(project(":xtmd-android-runtime"))
    api(platform(libs.compose.bom))
    api(libs.compose.ui)
    api(libs.compose.foundation)
    api(libs.compose.material3)

    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.robolectric)
    testImplementation(libs.junit)
    debugImplementation(libs.compose.ui.test.manifest)
}

// What its tests draw is made by hand: no native library is loaded.
tasks.withType<Test>().configureEach { maxHeapSize = "2g" }
