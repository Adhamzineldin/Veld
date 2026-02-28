# Publishing the Veld JetBrains Plugin

## Prerequisites

### 1. JetBrains Account
- Create account at https://account.jetbrains.com/
- This will be your publisher account

### 2. JetBrains Marketplace
- Visit https://plugins.jetbrains.com/
- Sign in with your JetBrains account
- Go to https://plugins.jetbrains.com/author/me
- Note your **Publisher ID**

### 3. Generate Personal Access Token
1. Go to https://plugins.jetbrains.com/author/me/tokens
2. Click **Generate New Token**
3. Name: "Plugin Publishing"
4. Scope: **Plugin Repository**
5. Copy the token (you won't see it again!)
6. Save as environment variable:
   ```bash
   export ORG_GRADLE_PROJECT_intellijPublishToken=YOUR_TOKEN_HERE
   ```

## Build & Test

### 1. Install Dependencies
```bash
cd editors/jetbrains
./gradlew dependencies
```

### 2. Build Plugin
```bash
./gradlew buildPlugin
# Creates: build/distributions/veld-jetbrains-0.1.0.zip
```

### 3. Verify Plugin
```bash
./gradlew verifyPlugin
# Checks compatibility and plugin structure
```

### 4. Test Locally
```bash
./gradlew runIde
# Opens IDE with plugin installed for testing
```

## Publish to Marketplace

### Option 1: Automatic Publishing
```bash
export ORG_GRADLE_PROJECT_intellijPublishToken=YOUR_TOKEN

./gradlew publishPlugin
```

### Option 2: Manual Upload
1. Build plugin: `./gradlew buildPlugin`
2. Go to https://plugins.jetbrains.com/plugin/add
3. Upload `build/distributions/veld-jetbrains-0.1.0.zip`
4. Fill in plugin details
5. Submit for review

### Option 3: Update Version First
```bash
# Update version in build.gradle.kts
# Then publish
./gradlew publishPlugin
```

## Pre-Publish Checklist

- [ ] Plugin builds without errors: `./gradlew buildPlugin`
- [ ] Plugin verifies successfully: `./gradlew verifyPlugin`
- [ ] Plugin works in test IDE: `./gradlew runIde`
- [ ] All features tested (syntax, completion, validation, actions)
- [ ] README.md is complete and accurate
- [ ] plugin.xml has correct version and changelog
- [ ] LICENSE file included
- [ ] Screenshots prepared (optional but recommended)
- [ ] Compatible IDEs tested (IntelliJ, WebStorm, etc.)

## Plugin Compatibility

The plugin is configured to work with:
- **Since Build**: 231 (2023.1)
- **Until Build**: 241.* (2024.1.*)

To update compatibility:
```kotlin
// In build.gradle.kts
tasks.patchPluginXml {
    sinceBuild.set("231")        // Minimum version
    untilBuild.set("241.*")      // Maximum version
}
```

## Version Numbering

Follow semantic versioning: `MAJOR.MINOR.PATCH`

```bash
# Update version in build.gradle.kts
version = "0.2.0"

# Or use gradle task
./gradlew publishPlugin -Pversion=0.2.0
```

## Post-Publish

### 1. Verify on Marketplace
- Visit: https://plugins.jetbrains.com/plugin/PLUGIN_ID/veld
- Check description, screenshots, version info
- Test "Install to IDE" button

### 2. Test Installation
Open any JetBrains IDE:
```
Settings/Preferences → Plugins → Marketplace
Search for "Veld" → Install
```

### 3. Announce
- Update main Veld README with marketplace link
- Announce on social media
- Update documentation

## Continuous Publishing (GitHub Actions)

Create `.github/workflows/publish-jetbrains.yml`:

```yaml
name: Publish JetBrains Plugin

on:
  push:
    tags:
      - 'jetbrains-v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup JDK
        uses: actions/setup-java@v3
        with:
          java-version: 17
          distribution: 'temurin'
      
      - name: Setup Gradle
        uses: gradle/gradle-build-action@v2
      
      - name: Build Plugin
        run: |
          cd editors/jetbrains
          ./gradlew buildPlugin
      
      - name: Verify Plugin
        run: |
          cd editors/jetbrains
          ./gradlew verifyPlugin
      
      - name: Publish Plugin
        env:
          ORG_GRADLE_PROJECT_intellijPublishToken: ${{ secrets.JETBRAINS_PUBLISH_TOKEN }}
        run: |
          cd editors/jetbrains
          ./gradlew publishPlugin
```

Add secret `JETBRAINS_PUBLISH_TOKEN` to GitHub repository settings.

## Plugin Signing (Optional but Recommended)

JetBrains allows plugin signing for verification:

### 1. Generate Certificate
```bash
# Generate private key
openssl genrsa -out private.key 4096

# Generate certificate
openssl req -new -x509 -key private.key -out certificate.crt -days 365
```

### 2. Configure in build.gradle.kts
```kotlin
tasks.signPlugin {
    certificateChain.set(File("certificate.crt").readText())
    privateKey.set(File("private.key").readText())
    password.set(System.getenv("PRIVATE_KEY_PASSWORD"))
}
```

### 3. Sign Before Publishing
```bash
./gradlew signPlugin
./gradlew publishPlugin
```

## Troubleshooting

### Error: "Invalid plugin structure"
- Check plugin.xml is in `src/main/resources/META-INF/`
- Verify all required elements are present
- Run `./gradlew verifyPlugin` for details

### Error: "Authentication failed"
- Verify your token is correct
- Make sure token has "Plugin Repository" scope
- Check environment variable: `echo $ORG_GRADLE_PROJECT_intellijPublishToken`

### Error: "Plugin already exists"
- You can't publish the same version twice
- Increment version number in build.gradle.kts
- Or delete the old version from marketplace (if not approved yet)

### Plugin not appearing in marketplace
- Check plugin review status at https://plugins.jetbrains.com/author/me
- First publication requires manual review (1-3 days)
- Subsequent updates are usually automatic

### Build fails with "Task 'instrumentCode' not found"
- Make sure you're using IntelliJ Gradle Plugin version 1.16.1+
- Clean build: `./gradlew clean buildPlugin`

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2026-02-28 | Initial release |

## Resources

- [IntelliJ Platform SDK](https://plugins.jetbrains.com/docs/intellij/welcome.html)
- [Plugin Publishing](https://plugins.jetbrains.com/docs/intellij/publishing-plugin.html)
- [JetBrains Marketplace](https://plugins.jetbrains.com/)
- [Plugin Development Guidelines](https://plugins.jetbrains.com/docs/intellij/plugin-development-guidelines.html)

## Support

For issues with the plugin:
- GitHub Issues: https://github.com/veld-dev/veld/issues
- Email: support@veld.dev
- Marketplace Comments: https://plugins.jetbrains.com/plugin/PLUGIN_ID/reviews

---

**Ready to publish!** 🚀

