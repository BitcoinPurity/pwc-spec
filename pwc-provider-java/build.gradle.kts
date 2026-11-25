plugins {
    `java-library`
    application
    `maven-publish`
}

group = "org.bitcoinpurity.pwc"
version = "0.2.0"

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
    }
    withSourcesJar()
}

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:5.13.4"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testImplementation("com.fasterxml.jackson.core:jackson-databind:2.20.0")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

// The platform names of Native.platform(); the published jar carries all of them (input/requirements.md §6.2).
val os = System.getProperty("os.name").lowercase().let {
    when {
        it.startsWith("mac") -> "macos"
        it.startsWith("windows") -> "windows"
        else -> it
    }
}
val arch = System.getProperty("os.arch").let { if (it == "amd64") "x86_64" else if (it == "arm64") "aarch64" else it }
val libraryFile = System.mapLibraryName("pwc_provider_jni")
// A directory with prebuilt libraries as <os>-<arch>/<library>, used instead of building this platform's library.
val nativeLibs = findProperty("nativeLibs")?.toString()

val cargoBuild by tasks.registering(Exec::class) {
    description = "Builds the JNI library with cargo."
    workingDir = file("native")
    // CARGO overrides the cargo found on PATH, for environments where the JVM resolves PATH differently.
    commandLine(System.getenv("CARGO") ?: "cargo", "build", "--release", "--locked")
    inputs.files(fileTree("native/src"), "native/Cargo.toml", "native/Cargo.lock")
    inputs.files(fileTree("../pwc-provider/src"), "../pwc-provider/Cargo.toml")
    outputs.file("native/target/release/$libraryFile")
}

tasks.processResources {
    if (nativeLibs != null) {
        from(nativeLibs) {
            into("org/bitcoinpurity/pwc/provider/native")
        }
    } else {
        from(cargoBuild) {
            into("org/bitcoinpurity/pwc/provider/native/$os-$arch")
        }
    }
}

val nativeAccess = "--enable-native-access=ALL-UNNAMED"

tasks.test {
    useJUnitPlatform()
    jvmArgs(nativeAccess)
    systemProperty("pwc.testkit", file("../pwc-testkit").absolutePath)
    testLogging {
        events("failed")
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
}

tasks.withType<JavaCompile> {
    options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))
}

application {
    mainModule = "org.bitcoinpurity.pwc.provider"
    mainClass = "org.bitcoinpurity.pwc.provider.demo.Main"
    applicationDefaultJvmArgs = listOf("--enable-native-access=org.bitcoinpurity.pwc.provider")
}

publishing {
    publications {
        create<MavenPublication>("sdk") {
            from(components["java"])
            pom {
                name = "PWC Provider SDK for Java"
                description = "The Rust PWC Provider (Core v0.2.1 over LOCAL v0.1.0) for Java host wallets, through JNI"
                url = "https://github.com/BitcoinPurity/pwc-spec"
                licenses {
                    license {
                        name = "MIT License"
                        url = "https://opensource.org/license/mit"
                    }
                }
                scm {
                    url = "https://github.com/BitcoinPurity/pwc-spec"
                }
            }
        }
    }
    repositories {
        // CI publishes with a token from the environment (input/requirements.md §6.2).
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/BitcoinPurity/pwc-spec")
            credentials {
                username = System.getenv("PWC_PACKAGES_USER")
                password = System.getenv("PWC_PACKAGES_TOKEN")
            }
        }
    }
}
