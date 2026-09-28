plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("com.google.devtools.ksp") version "2.0.20-1.0.24"
}

android {
    namespace = "com.wabridge.source"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.wabridge.source"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
        externalNativeBuild {
            cmake {
                arguments += "-DWABRIDGE_CORE_DIR=${project.rootProject.projectDir.parentFile.parentFile.parentFile}/core"
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isDebuggable = true
            applicationIdSuffix = ".debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    // The Compose compiler is supplied by the org.jetbrains.kotlin.plugin.compose
    // plugin (versioned in the root build.gradle.kts). The legacy
    // composeOptions.kotlinCompilerExtensionVersion block is not used from Kotlin 2.0.

    packaging {
        resources.excludes.addAll(
            listOf("META-INF/{AL2.0,LGPL2.1,LICENSE_*.txt,LICENSE_*.html}",
            "META-INF/DEPENDENCIES")
        )
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            // 3.22.3 is not published in the Android SDK repository; 3.31.6 is the
            // newest 3.x available and satisfies cmake_minimum_required(3.22.3).
            version = "3.31.6"
        }
    }

    sourceSets {
        getByName("main") {
            manifest.srcFile("src/main/AndroidManifest.xml")
            java.srcDirs("src/main/kotlin")
            res.srcDirs("src/main/res")
            assets.srcDirs("src/main/assets")
        }
    }

    lint {
        targetSdk = 35
    }
}

dependencies {
    // Core Android
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.4")

    // Jetpack Compose
    // NOTE: 1.6.11 was never published; the 1.6.x line ends at 1.6.8.
    implementation("androidx.activity:activity-compose:1.9.0")
    implementation("androidx.compose.ui:ui:1.6.8")
    implementation("androidx.compose.ui:ui-tooling-preview:1.6.8")
    implementation("androidx.compose.material3:material3:1.3.1")
    implementation("androidx.compose.material:material-icons-core:1.7.8")
    implementation("androidx.compose.material:material-icons-extended:1.7.8")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.4")

    // View-based Material Components: supplies the Theme.Material3.* XML themes
    // referenced by res/values/themes.xml. Compose's material3 artifact does
    // not provide these.
    implementation("com.google.android.material:material:1.12.0")

    // WorkManager (for background transfer orchestration)
    implementation("androidx.work:work-runtime-ktx:2.9.1")

    // Kotlin Coroutines
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.1")

    // JSON manifest serialization (InventoryEngine.buildManifestJson)
    implementation("com.google.code.gson:gson:2.11.0")

    // Testing
    testImplementation("junit:junit:4.13.2")
    testImplementation("io.mockk:mockk:1.13.11")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    debugImplementation("androidx.compose.ui:ui-tooling:1.6.8")
}
