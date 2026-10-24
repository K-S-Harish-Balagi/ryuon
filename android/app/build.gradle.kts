import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

// The release key never lives in the repository. Locally it is read from
// ~/.local/share/ryuon-signing/android/keystore.properties; in CI from environment variables.
// Without either, the release build is left unsigned.
// Read through Gradle's providers so the configuration cache notices when the key appears.
val signingProps = Properties().apply {
    val local = File(System.getProperty("user.home"), ".local/share/ryuon-signing/android/keystore.properties")
    providers.fileContents(objects.fileProperty().fileValue(local)).asText.orNull?.let { load(it.reader()) }
    providers.environmentVariable("RYUON_KEYSTORE").orNull?.let { setProperty("storeFile", it) }
    providers.environmentVariable("RYUON_KEYSTORE_PASSWORD").orNull?.let { setProperty("storePassword", it) }
    providers.environmentVariable("RYUON_KEY_ALIAS").orNull?.let { setProperty("keyAlias", it) }
    providers.environmentVariable("RYUON_KEY_PASSWORD").orNull?.let { setProperty("keyPassword", it) }
}
val hasReleaseKey = signingProps.getProperty("storeFile")?.let { File(it).exists() } == true

android {
    namespace = "io.github.k_s_harish_balagi.ryuon"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.k_s_harish_balagi.ryuon"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-dev"
    }

    signingConfigs {
        if (hasReleaseKey) {
            create("release") {
                storeFile = File(signingProps.getProperty("storeFile"))
                storePassword = signingProps.getProperty("storePassword")
                keyAlias = signingProps.getProperty("keyAlias")
                keyPassword = signingProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Light APK: strip unused code and resources
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (hasReleaseKey) signingConfig = signingConfigs.getByName("release")
        }
        debug {
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
        aidl = false
        buildConfig = false
        shaders = false
    }
    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
    // Only the languages Ryuon is translated into (English for now) ship in the APK
    androidResources {
        localeFilters += listOf("en")
    }
    testOptions {
        unitTests.all {
            // The fixtures shared with the desktop app's Rust tests
            it.systemProperty("ryuon.fixtures", rootProject.file("../tests/fixtures/ryuon-fixtures.json").absolutePath)
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui.tooling.preview)
    debugImplementation(libs.androidx.compose.ui.tooling)

    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)

    testImplementation(libs.junit)
    testImplementation(libs.json)
}
