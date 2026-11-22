import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType
import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.1.10"
    id("org.jetbrains.intellij.platform") version "2.14.0"
}

group = "dev.veld"
version = "2.0.0"

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        // Compile against the oldest supported platform (2024.3); verify against newer ones.
        create(IntelliJPlatformType.IntellijIdeaCommunity, "2024.3.5")
        bundledPlugin("org.jetbrains.plugins.textmate")
        bundledPlugin("com.intellij.modules.json")
        plugin("com.redhat.devtools.lsp4ij", "0.19.4")
        testFramework(TestFrameworkType.Platform)
        pluginVerifier()
    }
    testImplementation("junit:junit:4.13.2")
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion {
            sinceBuild = "243"
            untilBuild = provider { null }
        }
    }
    pluginVerification {
        ides {
            create(IntelliJPlatformType.IntellijIdeaCommunity, "2024.3.5")
        }
    }
}

// IntelliJ 2024.3 runs on JDK 21.
java {
    toolchain { languageVersion = JavaLanguageVersion.of(21) }
}
kotlin {
    compilerOptions { jvmTarget = JvmTarget.JVM_21 }
}

tasks.test {
    useJUnit()
}
