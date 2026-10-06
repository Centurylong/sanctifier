use zed_extension_api as zed;

struct SanctifierExtension;

impl zed::Extension for SanctifierExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = worktree.which("sanctifier").ok_or_else(|| {
            "Sanctifier was not found on PATH. Install it and make sure the sanctifier executable is visible to Zed."
                .to_string()
        })?;

        Ok(zed::Command {
            command,
            args: vec!["lsp".to_string(), "--stdio".to_string()],
            env: Vec::new(),
        })
    }
}

zed::register_extension!(SanctifierExtension);
