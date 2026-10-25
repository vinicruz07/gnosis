use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vault {
    pub name: String,
    pub path: PathBuf,
}

impl Vault {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() || !path.is_dir() {
            return Err(CoreError::NotFound(format!(
                "Diretório do Vault não encontrado: {}",
                path.display()
            )));
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled Vault")
            .to_string();

        Ok(Self { name, path })
    }
}
