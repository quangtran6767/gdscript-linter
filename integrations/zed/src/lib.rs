use zed_extension_api as zed;

struct Gdguard;
impl zed::Extension for Gdguard {
    fn new() -> Self {
        Self
    }
    fn language_server_command(
        &mut self,
        _id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = worktree
            .which("gdguard")
            .or_else(|| worktree.which("gdscript-formatter"))
            .ok_or_else(|| {
                "Install gdguard on PATH before starting its language server".to_string()
            })?;
        Ok(zed::Command {
            command,
            args: vec!["lsp".to_string()],
            env: worktree.shell_env(),
        })
    }
}
zed::register_extension!(Gdguard);
