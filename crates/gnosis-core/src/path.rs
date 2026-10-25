//! Tipo de caminho seguro e canônico para o Vault (`VaultPath`).
//!
//! Garante imunidade contra ataques de Path Traversal (`../`), normaliza separadores
//! de sistema operacional para `/` e impede o uso de caracteres proibidos.

use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Nomes reservados em sistemas baseados em Windows que devem ser rejeitados.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Representa um caminho relativo sanitizado e normalizado dentro de um Vault.
///
/// Características invariantes:
/// 1. Nunca contém componentes `..` (impossível escapar da raiz do Vault).
/// 2. Utiliza sempre `/` como separador de diretórios (compatível com Wikilinks, Windows, Linux, Mac e Web).
/// 3. Nunca inicia ou termina com `/`.
/// 4. Nunca está vazio.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VaultPath {
    inner: String,
}

impl VaultPath {
    /// Faz o parse e sanitização estrita de uma string de caminho relativo.
    ///
    /// # Erros
    /// Retorna `CoreError::PathTraversal` se contiver `..` ou tentativas de fuga.
    /// Retorna `CoreError::InvalidPath` se for vazio, contiver caracteres proibidos ou nomes reservados.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self> {
        let input = raw.as_ref().trim();

        if input.is_empty() {
            return Err(CoreError::InvalidPath(
                input.to_string(),
                "caminho não pode ser vazio",
            ));
        }

        // Rejeita tentativas explícitas de drive letters do Windows (ex: "C:\...")
        if input.len() >= 2 {
            let bytes = input.as_bytes();
            let is_drive_letter = bytes[0].is_ascii_alphabetic();
            let has_colon = bytes[1] == b':';
            if is_drive_letter && has_colon {
                return Err(CoreError::PathTraversal(input.to_string()));
            }
        }

        let mut normalized_parts: Vec<String> = Vec::new();

        // Quebra tanto por '/' quanto por '\' para compatibilidade total entre SOs
        for segment in input.split(|c| c == '/' || c == '\\') {
            let seg = segment.trim();

            if seg.is_empty() || seg == "." {
                continue;
            }

            if seg == ".." {
                return Err(CoreError::PathTraversal(input.to_string()));
            }

            // Validação de caracteres proibidos em sistemas de arquivos modernos
            if seg.contains(|c: char| {
                c.is_control()
                    || c == '<'
                    || c == '>'
                    || c == ':'
                    || c == '"'
                    || c == '|'
                    || c == '?'
                    || c == '*'
            }) {
                return Err(CoreError::InvalidPath(
                    input.to_string(),
                    "contém caracteres ilegais para sistema de arquivos (<, >, :, \", |, ?, *)",
                ));
            }

            // Validação de nomes reservados do Windows (mesmo em maiúsculas ou minúsculas)
            let stem = seg.split('.').next().unwrap_or(seg).to_ascii_uppercase();
            if WINDOWS_RESERVED_NAMES.contains(&stem.as_str()) {
                return Err(CoreError::InvalidPath(
                    input.to_string(),
                    "nome reservado de sistema de arquivos não permitido",
                ));
            }

            normalized_parts.push(seg.to_string());
        }

        if normalized_parts.is_empty() {
            return Err(CoreError::InvalidPath(
                input.to_string(),
                "caminho não possui segmentos válidos",
            ));
        }

        let inner = normalized_parts.join("/");
        Ok(Self { inner })
    }

    /// Retorna o caminho como uma fatia de string normalizada (`&str`).
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Retorna o nome final do arquivo ou pasta (ex: `"nota.md"` em `"pasta/nota.md"`).
    pub fn file_name(&self) -> Option<&str> {
        self.inner.rsplit('/').next()
    }

    /// Retorna o nome sem a extensão (ex: `"nota"` em `"pasta/nota.md"`).
    pub fn file_stem(&self) -> Option<&str> {
        let name = self.file_name()?;
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => Some(stem),
            _ => Some(name),
        }
    }

    /// Retorna a extensão do arquivo em letras minúsculas (ex: `"md"` em `"nota.MD"`).
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name()?;
        name.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// Retorna se o arquivo possui extensão Markdown (`.md` ou `.markdown`).
    pub fn is_markdown(&self) -> bool {
        match self.extension() {
            Some(ext) => ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"),
            None => false,
        }
    }

    /// Retorna o caminho do diretório pai, se existir.
    pub fn parent(&self) -> Option<Self> {
        match self.inner.rsplit_once('/') {
            Some((parent_str, _)) => Some(Self {
                inner: parent_str.to_string(),
            }),
            None => None,
        }
    }

    /// Concatena um sub-caminho de forma segura, garantindo que não escape do Vault.
    pub fn join(&self, subpath: impl AsRef<str>) -> Result<Self> {
        let combined = format!("{}/{}", self.inner, subpath.as_ref());
        Self::parse(combined)
    }
}

impl fmt::Debug for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VaultPath(\"{}\")", self.inner)
    }
}

impl fmt::Display for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl AsRef<str> for VaultPath {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_paths_normalization() -> std::result::Result<(), CoreError> {
        let p1 = VaultPath::parse("notas/projeto.md")?;
        assert_eq!(p1.as_str(), "notas/projeto.md");
        assert_eq!(p1.file_name(), Some("projeto.md"));
        assert_eq!(p1.file_stem(), Some("projeto"));
        assert_eq!(p1.extension(), Some("md"));
        assert!(p1.is_markdown());

        // Normalização de barras invertidas do Windows
        let p2 = VaultPath::parse(r"pastas\subpasta\documento.md")?;
        assert_eq!(p2.as_str(), "pastas/subpasta/documento.md");

        // Normalização de barras extras no início ou fim
        let p3 = VaultPath::parse("///pasta/outra_pasta///")?;
        assert_eq!(p3.as_str(), "pasta/outra_pasta");

        Ok(())
    }

    #[test]
    fn test_path_traversal_prevention() {
        assert!(VaultPath::parse("../fora_do_vault.md").is_err());
        assert!(VaultPath::parse("pasta/../../escapar.md").is_err());
        assert!(VaultPath::parse(r"pasta\..\..\escapar.md").is_err());
        assert!(VaultPath::parse("C:/Windows/System32").is_err());
    }

    #[test]
    fn test_invalid_characters_and_reserved_names() {
        assert!(VaultPath::parse("nota:invalida.md").is_err());
        assert!(VaultPath::parse("nota?interrogacao.md").is_err());
        assert!(VaultPath::parse("CON.md").is_err());
        assert!(VaultPath::parse("aux.txt").is_err());
        assert!(VaultPath::parse("").is_err());
        assert!(VaultPath::parse("   ").is_err());
    }

    #[test]
    fn test_parent_and_join() -> std::result::Result<(), CoreError> {
        let p = VaultPath::parse("projetos/rust/gnosis.md")?;
        let parent = match p.parent() {
            Some(parent) => parent,
            None => {
                return Err(CoreError::InvalidPath(
                    p.to_string(),
                    "esperava diretório pai",
                ))
            }
        };
        assert_eq!(parent.as_str(), "projetos/rust");

        let joined = parent.join("novo_arquivo.md")?;
        assert_eq!(joined.as_str(), "projetos/rust/novo_arquivo.md");

        // Teste de join com tentativa de fuga
        assert!(parent.join("../tentativa_fuga.md").is_err());

        Ok(())
    }
}
