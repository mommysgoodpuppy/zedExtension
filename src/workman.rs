use std::env;
use std::path::{Path, PathBuf};

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, LanguageServerId, Result,
};

struct WorkmanExtension;

impl WorkmanExtension {
    fn resolve_deno_binary(
        &self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Ok(lsp_settings) = LspSettings::for_worktree("workman-lsp", worktree) {
            if let Some(binary) = lsp_settings.binary {
                if let Some(path) = binary.path {
                    return Ok(path);
                }
            }
        }

        worktree
            .which("deno")
            .ok_or_else(|| format!("{language_server_id}: could not find deno on PATH"))
    }

    fn resolve_server_paths(&self, worktree: &zed::Worktree) -> Result<(String, String)> {
        if let Ok(lsp_settings) = LspSettings::for_worktree("workman-lsp", worktree) {
            if let Some(settings) = lsp_settings.settings {
                if let Some(server_path) = settings
                    .get("serverPath")
                    .and_then(|value| value.as_str())
                {
                    let server_path = PathBuf::from(server_path);
                    let deno_config = settings
                        .get("denoConfig")
                        .and_then(|value| value.as_str())
                        .map(PathBuf::from)
                        .unwrap_or_else(|| {
                            server_path
                                .parent()
                                .and_then(Path::parent)
                                .map(|parent| parent.join("deno.json"))
                                .unwrap_or_else(|| PathBuf::from("deno.json"))
                        });
                    return Ok((
                        deno_config.to_string_lossy().to_string(),
                        server_path.to_string_lossy().to_string(),
                    ));
                }

                if let Some(server_root) = settings
                    .get("serverRoot")
                    .and_then(|value| value.as_str())
                {
                    return Ok(self.paths_from_root(PathBuf::from(server_root)));
                }
            }
        }

        if let Ok(server_root) = env::var("WORKMAN_ROOT") {
            return Ok(self.paths_from_root(PathBuf::from(server_root)));
        }

        self.paths_from_worktree(worktree)
    }

    fn paths_from_root(&self, root: PathBuf) -> (String, String) {
        let deno_config = root.join("lsp").join("server").join("deno.json");
        let server_path = root
            .join("lsp")
            .join("server")
            .join("src")
            .join("server.ts");
        (
            deno_config.to_string_lossy().to_string(),
            server_path.to_string_lossy().to_string(),
        )
    }

    fn paths_from_worktree(&self, worktree: &zed::Worktree) -> Result<(String, String)> {
        let (deno_config, server_path) = self.paths_from_root(PathBuf::from(worktree.root_path()));
        if worktree
            .read_text_file("lsp/server/src/server.ts")
            .is_err()
        {
            return Err(format!(
                "Workman LSP not found at {}",
                server_path
            ));
        }
        Ok((deno_config, server_path))
    }
}

impl zed::Extension for WorkmanExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let deno = self.resolve_deno_binary(language_server_id, worktree)?;
        let (deno_config, server_path) = self.resolve_server_paths(worktree)?;

        let args = if let Ok(lsp_settings) = LspSettings::for_worktree("workman-lsp", worktree) {
            if let Some(binary) = lsp_settings.binary {
                if let Some(arguments) = binary.arguments {
                    arguments
                } else {
                    vec![
                        "run".to_string(),
                        "--allow-all".to_string(),
                        "--config".to_string(),
                        deno_config,
                        server_path,
                    ]
                }
            } else {
                vec![
                    "run".to_string(),
                    "--allow-all".to_string(),
                    "--config".to_string(),
                    deno_config,
                    server_path,
                ]
            }
        } else {
            vec![
                "run".to_string(),
                "--allow-all".to_string(),
                "--config".to_string(),
                deno_config,
                server_path,
            ]
        };

        let env = match zed::current_platform().0 {
            zed::Os::Mac | zed::Os::Linux => worktree.shell_env(),
            zed::Os::Windows => Default::default(),
        };

        Ok(zed::Command {
            command: deno,
            args,
            env,
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree("workman-lsp", worktree)
            .ok()
            .and_then(|settings| settings.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree("workman-lsp", worktree)
            .ok()
            .and_then(|settings| settings.settings))
    }
}

zed::register_extension!(WorkmanExtension);
