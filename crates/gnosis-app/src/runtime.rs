//! Utilitários de runtime, resiliência gráfica e logging persistente do Gnosis.
//!
//! Garante inicialização segura em qualquer hardware (incluindo GPUs legadas
//! sem suporte a OpenGL moderno) com fallback automático para `winit-software`,
//! além de persistência de preferência de renderizador e log em arquivo.

#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info, warn};

/// Modo de seleção do renderizador Slint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RenderBackendMode {
    /// Tenta GPU (FemtoVG/OpenGL) e faz fallback automático para Software (CPU) se falhar.
    #[default]
    Auto,
    /// Força renderização por GPU (FemtoVG/OpenGL).
    Gpu,
    /// Força renderização por Software (CPU) — ideal para hardware legado ou VMs.
    Software,
}

impl RenderBackendMode {
    /// Converte texto salvo/UI para `RenderBackendMode` com fallback seguro para `Auto`.
    pub fn from_str_lossy(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "gpu" | "femtovg" => Self::Gpu,
            "software" | "cpu" | "winit-software" => Self::Software,
            _ => Self::Auto,
        }
    }

    /// Identificador canônico serializável.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Gpu => "gpu",
            Self::Software => "software",
        }
    }

    /// Rótulo legível para exibição na interface.
    pub fn display_label(self) -> &'static str {
        match self {
            Self::Auto => "Automático (GPU → CPU)",
            Self::Gpu => "GPU (FemtoVG / OpenGL)",
            Self::Software => "Software (CPU / Legado)",
        }
    }

    /// Alterna para o próximo modo na ordem: Auto -> Software -> Gpu -> Auto.
    pub fn cycle_next(self) -> Self {
        match self {
            Self::Auto => Self::Software,
            Self::Software => Self::Gpu,
            Self::Gpu => Self::Auto,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RuntimePreferences {
    renderer_mode: RenderBackendMode,
}

/// Retorna o diretório de configuração/dados locais do Gnosis.
pub fn app_data_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            return PathBuf::from(local_app_data).join("Gnosis");
        }
    }
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg_config.trim().is_empty() {
            return PathBuf::from(xdg_config).join("gnosis");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".config").join("gnosis");
        }
    }
    PathBuf::from(".gnosis")
}

/// Caminho do arquivo de preferência de runtime (`runtime.json`).
fn preferences_file_path() -> PathBuf {
    app_data_dir().join("runtime.json")
}

/// Caminho do arquivo de log persistente (`gnosis.log`).
pub fn log_file_path() -> PathBuf {
    app_data_dir().join("gnosis.log")
}

/// Carrega a preferência de renderizador salva localmente (padrão: `Auto`).
pub fn load_renderer_preference() -> RenderBackendMode {
    let path = preferences_file_path();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return RenderBackendMode::Auto;
    };
    serde_json::from_str::<RuntimePreferences>(&raw)
        .map(|prefs| prefs.renderer_mode)
        .unwrap_or_default()
}

/// Salva a preferência de renderizador localmente.
pub fn save_renderer_preference(mode: RenderBackendMode) -> Result<(), String> {
    let dir = app_data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Falha ao criar diretório de config: {e}"))?;
    let prefs = RuntimePreferences {
        renderer_mode: mode,
    };
    let json = serde_json::to_string_pretty(&prefs)
        .map_err(|e| format!("Falha ao serializar preferência: {e}"))?;
    std::fs::write(preferences_file_path(), json)
        .map_err(|e| format!("Falha ao gravar preferência: {e}"))
}

/// Inicializa o sistema de logs escrevendo em arquivo local (`gnosis.log`) e stderr.
pub fn init_logging() {
    let log_path = log_file_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file_res = OpenOptions::new().create(true).append(true).open(&log_path);
    match file_res {
        Ok(file) => {
            let file_arc = std::sync::Mutex::new(file);
            let _ = tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(move || -> Box<dyn Write + Send> {
                    if let Ok(guard) = file_arc.lock() {
                        if let Ok(cloned) = guard.try_clone() {
                            return Box::new(cloned);
                        }
                    }
                    Box::new(std::io::stderr())
                })
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt().try_init();
        }
    }
}

/// Aplica `SLINT_BACKEND` antes de instanciar a janela com base em CLI ou preferência salva.
///
/// Retorna o modo efetivo configurado e uma descrição amigável do motor gráfico.
pub fn configure_renderer_env() -> (RenderBackendMode, &'static str) {
    // Se já veio via variável de ambiente (ex.: fallback ou override do usuário), respeita.
    if let Ok(env_backend) = std::env::var("SLINT_BACKEND") {
        let lower = env_backend.to_ascii_lowercase();
        if lower.contains("software") {
            return (
                RenderBackendMode::Software,
                "Slint Software Renderer (CPU / Fallback ativo)",
            );
        }
        return (
            RenderBackendMode::Gpu,
            "Slint FemtoVG (OpenGL / Definido por ambiente)",
        );
    }

    // Verifica flag opcional de linha de comando: `--renderer=software|gpu|auto`
    let cli_mode = std::env::args().find_map(|arg| {
        arg.strip_prefix("--renderer=")
            .map(RenderBackendMode::from_str_lossy)
    });

    let mode = cli_mode.unwrap_or_else(load_renderer_preference);
    match mode {
        RenderBackendMode::Software => {
            std::env::set_var("SLINT_BACKEND", "winit-software");
            (
                RenderBackendMode::Software,
                "Slint Software Renderer (CPU / Modo Legado)",
            )
        }
        RenderBackendMode::Gpu => {
            std::env::set_var("SLINT_BACKEND", "winit-femtovg");
            (RenderBackendMode::Gpu, "Slint FemtoVG (OpenGL)")
        }
        RenderBackendMode::Auto => (
            RenderBackendMode::Auto,
            "Slint FemtoVG / Auto-Fallback Software",
        ),
    }
}

/// Tenta executar uma etapa gráfica (ex.: `AppWindow::new()` ou `app.run()`); se falhar
/// em modo `Auto` (como erro de contexto OpenGL no `winit`/`glutin`), relança o processo
/// automaticamente com `SLINT_BACKEND=winit-software`.
pub fn run_or_fallback_to_software<T, E, F>(op: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
    E: std::fmt::Display,
{
    match op() {
        Ok(val) => Ok(val),
        Err(err) => {
            let already_fallback = std::env::var("GNOSIS_SOFTWARE_FALLBACK").is_ok();
            let explicit_backend = std::env::var("SLINT_BACKEND").is_ok();

            if !already_fallback && !explicit_backend {
                warn!(
                    "Falha ao inicializar renderizador GPU ({err}). Relançando em modo Software (winit-software)..."
                );

                if let Ok(exe) = std::env::current_exe() {
                    let args: Vec<_> = std::env::args_os().skip(1).collect();
                    match Command::new(&exe)
                        .args(args)
                        .env("SLINT_BACKEND", "winit-software")
                        .env("GNOSIS_SOFTWARE_FALLBACK", "1")
                        .spawn()
                    {
                        Ok(_child) => {
                            info!("Instância em modo Software iniciada; encerrando processo inicial.");
                            std::process::exit(0);
                        }
                        Err(spawn_err) => {
                            error!("Falha ao relançar Gnosis em modo Software: {spawn_err}");
                        }
                    }
                }
            } else {
                error!("Falha fatal ao inicializar janela Slint: {err}");
            }

            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_mode_parsing_and_cycling() {
        assert_eq!(RenderBackendMode::from_str_lossy("auto"), RenderBackendMode::Auto);
        assert_eq!(RenderBackendMode::from_str_lossy("GPU"), RenderBackendMode::Gpu);
        assert_eq!(
            RenderBackendMode::from_str_lossy("winit-software"),
            RenderBackendMode::Software
        );
        assert_eq!(
            RenderBackendMode::from_str_lossy("desconhecido"),
            RenderBackendMode::Auto
        );

        assert_eq!(RenderBackendMode::Auto.cycle_next(), RenderBackendMode::Software);
        assert_eq!(RenderBackendMode::Software.cycle_next(), RenderBackendMode::Gpu);
        assert_eq!(RenderBackendMode::Gpu.cycle_next(), RenderBackendMode::Auto);
    }
}
