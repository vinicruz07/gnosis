use std::path::PathBuf;
use crate::error::{NativeFsError, Result};

#[derive(Debug, Clone)]
pub enum AppMode {
    Desktop,
    Portable,
}

#[derive(Debug, Clone)]
pub struct StoragePaths {
    pub mode: AppMode,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl StoragePaths {
    pub fn resolve(force_portable: bool) -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .map_err(|_| NativeFsError::ExecutablePathError)?
            .parent()
            .ok_or(NativeFsError::ExecutablePathError)?
            .to_path_buf();

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
            let home = std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .ok_or(NativeFsError::ExecutablePathError)?;

            let base = home.join(".gnosis");
            Ok(Self {
                mode: AppMode::Desktop,
                config_dir: base.join("config"),
                data_dir: base.join("data"),
                cache_dir: base.join("cache"),
            })
        }
    }
}
