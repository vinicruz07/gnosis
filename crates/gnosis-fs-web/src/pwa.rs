//! Validador e utilitários de empacotamento estático PWA (HTML5 + WASM + Service Worker).
//!
//! Garante que todos os artefatos estáticos necessários para inicialização autônoma
//! em modo avião (offline-first) estejam presentes, válidos e em conformidade
//! com os padrões W3C de Progressive Web Apps.

use crate::error::{Result, WebFsError};
use serde::{Deserialize, Serialize};

/// Estrutura representativa de um ícone no manifesto PWA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PwaIcon {
    pub src: String,
    pub sizes: String,
    #[serde(rename = "type")]
    pub icon_type: String,
    #[serde(default)]
    pub purpose: Option<String>,
}

/// Modelo estrutural do manifesto de Progressive Web App (W3C PWA Manifest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PwaManifest {
    pub name: String,
    pub short_name: String,
    pub description: String,
    pub start_url: String,
    pub scope: String,
    pub display: String,
    pub background_color: String,
    pub theme_color: String,
    pub orientation: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub icons: Vec<PwaIcon>,
}

impl PwaManifest {
    /// Analisa e valida uma string JSON de manifesto PWA.
    pub fn parse(json_str: &str) -> Result<Self> {
        let manifest: Self = serde_json::from_str(json_str)
            .map_err(|e| WebFsError::CorruptedPayload(format!("Manifesto PWA inválido: {}", e)))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Valida campos obrigatórios para instalação autônoma (modo standalone).
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(WebFsError::CorruptedPayload("Nome do PWA não pode ser vazio".to_string()));
        }
        if self.start_url.trim().is_empty() {
            return Err(WebFsError::CorruptedPayload("start_url do PWA não pode ser vazio".to_string()));
        }
        if self.display != "standalone" && self.display != "fullscreen" && self.display != "minimal-ui" {
            return Err(WebFsError::CorruptedPayload(format!(
                "Modo display '{}' incompatível para PWA independente",
                self.display
            )));
        }
        if self.icons.is_empty() {
            return Err(WebFsError::CorruptedPayload("O PWA deve conter ao menos um ícone".to_string()));
        }
        Ok(())
    }
}

/// Validador de conformidade e integridade do bundle estático PWA.
pub struct PwaBundleValidator;

impl PwaBundleValidator {
    /// Valida o shell HTML5 garantindo metadados essenciais de PWA e offline.
    pub fn validate_html_shell(html_content: &str) -> Result<()> {
        if !html_content.contains("<!DOCTYPE html>") {
            return Err(WebFsError::CorruptedPayload("Documento HTML sem DOCTYPE válido".to_string()));
        }
        if !html_content.contains("name=\"viewport\"") {
            return Err(WebFsError::CorruptedPayload("HTML sem meta tag viewport obrigatória para PWA".to_string()));
        }
        if !html_content.contains("rel=\"manifest\"") {
            return Err(WebFsError::CorruptedPayload("HTML sem link de manifesto PWA".to_string()));
        }
        if !html_content.contains("name=\"theme-color\"") {
            return Err(WebFsError::CorruptedPayload("HTML sem meta tag theme-color".to_string()));
        }
        Ok(())
    }

    /// Valida se o script do Service Worker contém lógica de cache-first e eventos essenciais.
    pub fn validate_service_worker(sw_content: &str) -> Result<()> {
        if !sw_content.contains("addEventListener('install'") {
            return Err(WebFsError::CorruptedPayload("Service Worker sem ouvinte de evento 'install'".to_string()));
        }
        if !sw_content.contains("addEventListener('activate'") {
            return Err(WebFsError::CorruptedPayload("Service Worker sem ouvinte de evento 'activate'".to_string()));
        }
        if !sw_content.contains("addEventListener('fetch'") {
            return Err(WebFsError::CorruptedPayload("Service Worker sem ouvinte de evento 'fetch'".to_string()));
        }
        if !sw_content.contains("caches.open") && !sw_content.contains("caches.match") {
            return Err(WebFsError::CorruptedPayload("Service Worker sem estratégia de cache offline".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r##"{
        "name": "Gnosis — Segundo Cérebro Local",
        "short_name": "Gnosis",
        "description": "Segundo cérebro pessoal 100% offline-first",
        "start_url": "./index.html",
        "scope": "./",
        "display": "standalone",
        "background_color": "#0d1117",
        "theme_color": "#8957e5",
        "icons": [
            {
                "src": "./icons/icon-192.svg",
                "sizes": "192x192",
                "type": "image/svg+xml"
            }
        ]
    }"##;

    #[test]
    fn test_parse_valid_pwa_manifest() -> Result<()> {
        let manifest = PwaManifest::parse(SAMPLE_MANIFEST)?;
        assert_eq!(manifest.name, "Gnosis — Segundo Cérebro Local");
        assert_eq!(manifest.short_name, "Gnosis");
        assert_eq!(manifest.display, "standalone");
        assert_eq!(manifest.icons.len(), 1);
        assert_eq!(manifest.icons[0].sizes, "192x192");
        Ok(())
    }

    #[test]
    fn test_invalid_pwa_manifest_missing_icons() {
        let invalid_json = r##"{
            "name": "Gnosis",
            "short_name": "Gnosis",
            "description": "Teste",
            "start_url": "./index.html",
            "scope": "./",
            "display": "standalone",
            "background_color": "#000",
            "theme_color": "#000",
            "icons": []
        }"##;

        let result = PwaManifest::parse(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_html_shell_validation() -> Result<()> {
        let sample_html = r##"
            <!DOCTYPE html>
            <html lang="pt-BR">
            <head>
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <meta name="theme-color" content="#8957e5">
                <link rel="manifest" href="./manifest.webmanifest">
            </head>
            <body></body>
            </html>
        "##;

        PwaBundleValidator::validate_html_shell(sample_html)?;
        Ok(())
    }

    #[test]
    fn test_service_worker_validation() -> Result<()> {
        let sample_sw = r#"
            self.addEventListener('install', (e) => { caches.open('v1'); });
            self.addEventListener('activate', (e) => { });
            self.addEventListener('fetch', (e) => { caches.match(e.request); });
        "#;

        PwaBundleValidator::validate_service_worker(sample_sw)?;
        Ok(())
    }

    #[test]
    fn test_real_web_artifacts_integrity() -> Result<()> {
        // Valida os arquivos estáticos criados na pasta web/
        let manifest_content = include_str!("../../../web/manifest.webmanifest");
        let html_content = include_str!("../../../web/index.html");
        let sw_content = include_str!("../../../web/sw.js");

        let manifest = PwaManifest::parse(manifest_content)?;
        assert_eq!(manifest.short_name, "Gnosis");
        assert_eq!(manifest.display, "standalone");
        assert!(manifest.icons.len() >= 2);

        PwaBundleValidator::validate_html_shell(html_content)?;
        PwaBundleValidator::validate_service_worker(sw_content)?;

        Ok(())
    }
}
