plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "io.github.danperks.normsdk"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    testOptions {
        unitTests.all {
            // JVM unit tests load the host build of the library (scripts/build.sh python).
            it.systemProperty("jna.library.path", rootDir.resolve("../python/normsdk").absolutePath)
        }
    }
}

kotlin {
    jvmToolchain(17)
}

dependencies {
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    testImplementation("net.java.dev.jna:jna:5.17.0")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.json:json:20240303")
}
