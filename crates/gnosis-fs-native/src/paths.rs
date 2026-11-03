//! Resolução de diretórios para modo Desktop e modo Portable autônomo.
//!
//! Em modo Portable:
//! - Nenhuma pasta fora do diretório do executável é acessada ou modificada.
//! - Dados, configurações e caches são gravados em `./data`, `./config` e `./cache`.
//!
//! Em modo Desktop:
//! - Utiliza os diretórios de usuário padrão do sistema operacional (`%USERPROFILE%/.gnosis`).

use crate::error::{NativeFsError, Result};
use std::path::PathBuf;

/// Modo de execução da aplicação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Desktop,
    Portable,
    Mobile,
}

/// Diretórios de armazenamento resolvidos para a aplicação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePaths {
    pub mode: AppMode,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl StoragePaths {
    /// Resolve os caminhos de armazenamento do Gnosis.
    ///
    /// Se `force_portable` for verdadeiro ou se existir o arquivo marcador `gnosis.portable`
    /// na mesma pasta do executável, opera em modo Portable isolado.
    /// Em ambiente móvel (Android Scoped Storage), isola os dados na pasta do app.
    pub fn resolve(force_portable: bool) -> Result<Self> {
        // Se variável de ambiente indicar diretório Android ou modo mobile explícito:
        if let Some(android_files) = std::env::var_os("GNOSIS_ANDROID_FILES_DIR") {
            return Ok(Self::resolve_mobile(PathBuf::from(android_files)));
        }

        #[cfg(target_os = "android")]
        {
            let default_android_dir = PathBuf::from("/data/data/org.gnosis.app/files");
            return Ok(Self::resolve_mobile(default_android_dir));
        }

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return Err(NativeFsError::ExecutablePathError),
        };

        let exe_dir = match exe_path.parent() {
            Some(p) => p.to_path_buf(),
            None => return Err(NativeFsError::ExecutablePathError),
        };

        let is_portable = force_portable || exe_dir.join("gnosis.portable").exists();

        if is_portable {
            let data_dir = exe_dir.join("data");
            let config_dir = exe_dir.join("config");
            let cache_dir = exe_dir.join("cache");

            Ok(Self {
                mode: AppMode::Portable,
                config_dir,
                data_dir,
                cache_dir,
            })
        } else {
            // Em modo desktop padrão, usa subpasta Gnosis no perfil do usuário
            let home_var = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
            let home = match home_var {
                Some(h) => PathBuf::from(h),
                None => return Err(NativeFsError::ExecutablePathError),
            };

            let base = home.join(".gnosis");
            Ok(Self {
                mode: AppMode::Desktop,
                config_dir: base.join("config"),
                data_dir: base.join("data"),
                cache_dir: base.join("cache"),
            })
        }
    }

    /// Retorna se a aplicação está operando em modo Portable.
    pub fn is_portable(&self) -> bool {
        self.mode == AppMode::Portable
    }

    /// Retorna se a aplicação está operando em modo Mobile (Android Scoped Storage).
    pub fn is_mobile(&self) -> bool {
        self.mode == AppMode::Mobile
    }

    /// Resolve os caminhos de armazenamento para a versão Mobile (Android Scoped Storage).
    ///
    /// No Android, todos os arquivos, Grimórios e caches são mantidos estritamente dentro do
    /// diretório de armazenamento privado do aplicativo (ex: `/data/user/0/org.gnosis.app/files`),
    /// garantindo isolamento total sem dependência de permissões perigosas do sistema.
    pub fn resolve_mobile(app_internal_dir: impl Into<PathBuf>) -> Self {
        let base = app_internal_dir.into();
        Self {
            mode: AppMode::Mobile,
            config_dir: base.join("config"),
            data_dir: base.join("data"),
            cache_dir: base.join("cache"),
        }
    }

    /// Cria as pastas de dados, configuração e cache caso não existam.
    pub fn ensure_dirs_exist(&self) -> Result<()> {
        if !self.config_dir.exists() {
            std::fs::create_dir_all(&self.config_dir)?;
        }
        if !self.data_dir.exists() {
            std::fs::create_dir_all(&self.data_dir)?;
        }
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portable_paths_resolution() -> Result<()> {
        let paths = StoragePaths::resolve(true)?;
        assert!(paths.is_portable());
        assert_eq!(paths.mode, AppMode::Portable);
        assert!(paths.data_dir.ends_with("data"));
        assert!(paths.config_dir.ends_with("config"));
        assert!(paths.cache_dir.ends_with("cache"));
        Ok(())
    }

    #[test]
    fn test_mobile_paths_resolution() {
        let dummy_android_dir = PathBuf::from("/data/user/0/org.gnosis.app/files");
        let paths = StoragePaths::resolve_mobile(&dummy_android_dir);
        assert!(paths.is_mobile());
        assert_eq!(paths.mode, AppMode::Mobile);
        assert_eq!(paths.data_dir, dummy_android_dir.join("data"));
        assert_eq!(paths.config_dir, dummy_android_dir.join("config"));
        assert_eq!(paths.cache_dir, dummy_android_dir.join("cache"));
    }
}
