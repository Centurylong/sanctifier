# Sanctifier for IntelliJ — skeleton

Placeholder for the IntelliJ plugin. The language server it will drive is
complete and running today; this directory records the integration shape so
whoever picks the plugin up is not starting from a blank page.

## What the plugin has to do

IntelliJ platform 2023.2+ ships `LspServerSupportProvider`, which means the
plugin is mostly a process descriptor — the platform owns the LSP client, the
diagnostics rendering, and the hover popup.

```kotlin
// src/main/kotlin/dev/sanctifier/SanctifierLspServerSupportProvider.kt
class SanctifierLspServerSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerSupportProvider.LspServerStarter,
    ) {
        if (file.extension != "rs") return
        serverStarter.ensureServerStarted(SanctifierLspServerDescriptor(project))
    }
}

private class SanctifierLspServerDescriptor(project: Project) :
    ProjectWideLspServerDescriptor(project, "Sanctifier") {

    override fun isSupportedFile(file: VirtualFile) = file.extension == "rs"

    // The CLI subcommand rather than the standalone binary: anyone who has
    // Sanctifier installed at all has this, and it is one thing to find on PATH.
    override fun createCommandLine() = GeneralCommandLine("sanctifier", "lsp", "--stdio")
}
```

Registered in `src/main/resources/META-INF/plugin.xml`:

```xml
<extensions defaultExtensionNs="com.intellij">
  <platform.lsp.serverSupportProvider
      implementation="dev.sanctifier.SanctifierLspServerSupportProvider"/>
</extensions>
```

## Open questions for whoever builds it

- **Which IDEs.** `platform.lsp` is a paid-tier API — it is present in IntelliJ
  IDEA Ultimate and CLion but not in Community. A Community build needs either
  a bundled LSP client or a direct PSI-based integration, which is a much
  larger piece of work. Worth deciding before writing code.
- **Binary discovery.** The descriptor above assumes `sanctifier` is on PATH.
  A settings panel with an explicit path, plus a clear "not found" notification,
  is the difference between a plugin that works and one that silently does
  nothing.
- **Gradle setup.** Not committed here on purpose; the IntelliJ Platform Gradle
  Plugin version has to be picked against the target IDE build range, and a
  stale one is worse than none.

## Testing against the server directly

The server does not need an IDE to exercise:

```sh
cargo build --release --manifest-path ../../tooling/sanctifier-lsp/Cargo.toml
python3 ../../scripts/lsp-conformance.py \
  ../../tooling/sanctifier-lsp/target/release/sanctifier-lsp
```
