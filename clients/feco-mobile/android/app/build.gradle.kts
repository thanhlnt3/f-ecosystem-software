plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "vn.feco.feco_mobile"
    // Android 16 (API 36), Google Play yêu cầu nhắm bản mới (mobile-launcher, design M2).
    compileSdk = 36
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "vn.feco.launcher"
        // Android 10 (API 29) trở lên: khoảng 95% máy Android đang dùng (11/2026).
        minSdk = 29
        targetSdk = 36
        // Địa chỉ chuyển hướng của AppAuth: vn.feco.launcher:/oauth2redirect.
        manifestPlaceholders += mapOf("appAuthRedirectScheme" to "vn.feco.launcher")
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // Chưa có khóa tải lên Google Play (QUESTIONS.md): bản phát hành ký bằng khóa debug,
            // chỉ để cài thử.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
