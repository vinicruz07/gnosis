//! Modelo formal do manifesto de plugins (`plugin.json`).
//!
//! Todo plugin do Gnosis deve conter um arquivo `plugin.json` na sua raiz
//! declarando metadados de identificação, ponto de entrada e permissões requeridas.

use crate::error::{PluginError, Result};
use serde::{Deserialize, Serialize};

/// Permissões explícitas solicitadas por um plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    /// Leitura de notas e documentos do Vault.
    ReadNotes,
    /// Criação, modificação e remoção de notas no Vault.
    WriteNotes,
    /// Registro de comandos na paleta do Quick Switcher (`Ctrl+O`).
    UiCommands,
    /// Injeção de botões na barra lateral ou status bar.
    UiWidgets,
    /// Acesso a rede externa (HTTP/WebSockets).
    Network,
    /// Renderização personalizada de blocos na AST do Markdown ou Canvas.
    CustomRenderer,
}

impl std::fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadNotes => write!(f, "read_notes"),
            Self::WriteNotes => write!(f, "write_notes"),
            Self::UiCommands => write!(f, "ui_commands"),
            Self::UiWidgets => write!(f, "ui_widgets"),
            Self::Network => write!(f, "network"),
            Self::CustomRenderer => write!(f, "custom_renderer"),
        }
    }
}

/// Manifesto descritivo de um plugin (`plugin.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Identificador único e imutável do plugin (ex: "word-counter", "pomodoro-timer").
    pub id: String,
    /// Nome amigável exibido ao usuário (ex: "Contador de Palavras Avançado").
    pub name: String,
    /// Versão do plugin no padrão SemVer (ex: "1.0.0").
    pub version: String,
    /// Descrição concisa da funcionalidade oferecida.
    pub description: String,
    /// Autor ou equipe mantenedora do plugin.
    pub author: String,
    /// Arquivo principal de execução (ex: "main.js", "plugin.wasm").
    pub entrypoint: String,
    /// Versão mínima do Gnosis necessária para execução.
    #[serde(default)]
    pub min_app_version: Option<String>,
    /// Permissões de acesso aos recursos do sistema requeridas pelo plugin.
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    /// Link opcional para o repositório ou documentação oficial.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Licença do código (ex: "MIT", "Apache-2.0").
    #[serde(default)]
    pub license: Option<String>,
}

impl PluginManifest {
    /// Cria um manifesto básico com os campos obrigatórios.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        author: impl Into<String>,
        entrypoint: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            author: author.into(),
            entrypoint: entrypoint.into(),
            min_app_version: None,
            permissions: Vec::new(),
            homepage: None,
            license: None,
        }
    }

    /// Define as permissões requeridas pelo plugin (padrão builder).
    pub fn with_permissions(mut self, permissions: Vec<PluginPermission>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Analisa e desserializa o manifesto a partir de uma string JSON, validando seus campos.
    pub fn from_json_str(json_str: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(json_str)
            .map_err(|e| PluginError::InvalidManifest(format!("JSON inválido: {}", e)))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Valida os campos do manifesto assegurando que não violem regras de integridade.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(PluginError::InvalidManifest("O ID do plugin não pode ser vazio".to_string()));
        }

        // Garante que o ID contenha apenas caracteres seguros para pastas
        if !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(PluginError::InvalidManifest(format!(
                "ID de plugin inválido: '{}'. Deve conter apenas caracteres alfanuméricos, '-' ou '_'",
                self.id
            )));
        }

        if self.name.trim().is_empty() {
            return Err(PluginError::InvalidManifest("O nome do plugin não pode ser vazio".to_string()));
        }

        if self.version.trim().is_empty() {
            return Err(PluginError::InvalidManifest("A versão do plugin não pode ser vazia".to_string()));
        }

        if self.entrypoint.trim().is_empty() {
            return Err(PluginError::InvalidManifest("O ponto de entrada (entrypoint) não pode ser vazio".to_string()));
        }

        // Impede path traversal no entrypoint
        if self.entrypoint.contains("..") || self.entrypoint.starts_with('/') || self.entrypoint.starts_with('\\') {
            return Err(PluginError::InvalidManifest(format!(
                "Ponto de entrada com caminho suspeito ou ilegal: '{}'",
                self.entrypoint
            )));
        }

        Ok(())
    }

    /// Retorna verdadeiro se o manifesto solicitou uma permissão específica.
    pub fn has_permission(&self, perm: PluginPermission) -> bool {
        self.permissions.contains(&perm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_manifest_parse() {
        let json = r#"{
            "id": "zen-mode",
            "name": "Zen Mode",
            "version": "1.2.0",
            "description": "Modo de foco total sem distrações visuais",
            "author": "Gnosis Community",
            "entrypoint": "zen.js",
            "permissions": ["ui_commands", "read_notes"],
            "license": "MIT"
        }"#;

        let manifest = PluginManifest::from_json_str(json);
        assert!(manifest.is_ok());
        if let Ok(m) = manifest {
            assert_eq!(m.id, "zen-mode");
            assert_eq!(m.name, "Zen Mode");
            assert_eq!(m.version, "1.2.0");
            assert!(m.has_permission(PluginPermission::UiCommands));
            assert!(m.has_permission(PluginPermission::ReadNotes));
            assert!(!m.has_permission(PluginPermission::Network));
        }
    }

    #[test]
    fn test_invalid_manifest_id() {
        let json = r#"{
            "id": "../fuga_diretorio",
            "name": "Malicioso",
            "version": "1.0.0",
            "description": "Tentativa de path traversal",
            "author": "Hacker",
            "entrypoint": "main.js"
        }"#;

        let manifest = PluginManifest::from_json_str(json);
        assert!(manifest.is_err());
    }

    #[test]
    fn test_invalid_entrypoint_path() {
        let json = r#"{
            "id": "safe-id",
            "name": "Safe Name",
            "version": "1.0.0",
            "description": "Desc",
            "author": "Author",
            "entrypoint": "../../etc/passwd"
        }"#;

        let manifest = PluginManifest::from_json_str(json);
        assert!(manifest.is_err());
    }
}
