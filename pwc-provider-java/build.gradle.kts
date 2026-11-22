plugins {
    `java-library`
    application
}

group = "org.bitcoinpurity.pwc"
version = "0.1.0"

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
    }
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

val os = System.getProperty("os.name").lowercase().let { if (it.startsWith("mac")) "macos" else it }
val arch = System.getProperty("os.arch").let { if (it == "amd64") "x86_64" else if (it == "arm64") "aarch64" else it }
val libraryFile = if (os == "macos") "libpwc_provider_jni.dylib" else "libpwc_provider_jni.so"

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
    from(cargoBuild) {
        into("org/bitcoinpurity/pwc/provider/native/$os-$arch")
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
    mainClass = "org.bitcoinpurity.pwc.provider.demo.Main"
    applicationDefaultJvmArgs = listOf(nativeAccess)
}
