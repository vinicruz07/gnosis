//! Sistema dinâmico de temas via TOML com estética filosófica e mística.
//!
//! Temas integrados:
//! - **Abyss** (O Abismo Cósmico): Escuridão profunda primordial para concentração absoluta.
//! - **Aether** (O Éter Celestial): O quinto elemento, luz pura, clareza e sutileza.
//! - **Arcana** (A Sabedoria Oculta): Púrpura hermética e alquimia mental.
//! - **Boreas** (O Vento Boreal): Serenidade gélida do norte ártico e reflexão límpida.

use serde::{Deserialize, Serialize};

/// Definição completa da paleta cromática de um tema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeColors {
    pub name: String,
    pub description: String,
    pub background: String,
    pub sidebar_bg: String,
    pub surface: String,
    pub surface_hover: String,
    pub surface_active: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub accent: String,
    pub border: String,
    pub success: String,
    pub warning: String,
    pub danger: String,
}

impl ThemeColors {
    /// Tema escuro cósmico: Abyss
    pub fn abyss() -> Self {
        Self {
            name: "Abyss".to_string(),
            description: "O Abismo Cósmico — O vazio primordial para foco e introspecção profunda".to_string(),
            background: "#111116".to_string(),
            sidebar_bg: "#090a0f".to_string(),
            surface: "#1b1c24".to_string(),
            surface_hover: "#252733".to_string(),
            surface_active: "#323545".to_string(),
            text_primary: "#e2e4ee".to_string(),
            text_secondary: "#9ca0b0".to_string(),
            text_muted: "#5a5e70".to_string(),
            accent: "#818cf8".to_string(), // Violeta estelar
            border: "#232533".to_string(),
            success: "#a6e3a1".to_string(),
            warning: "#f9e2af".to_string(),
            danger: "#f38ba8".to_string(),
        }
    }

    /// Tema claro celestial: Aether
    pub fn aether() -> Self {
        Self {
            name: "Aether".to_string(),
            description: "O Éter Celestial — O quinto elemento primordial, luz translúcida e pureza do pensamento".to_string(),
            background: "#f8f9fa".to_string(),
            sidebar_bg: "#f0f2f5".to_string(),
            surface: "#e4e7ec".to_string(),
            surface_hover: "#d2d6dc".to_string(),
            surface_active: "#c1c7d0".to_string(),
            text_primary: "#1d2939".to_string(),
            text_secondary: "#475467".to_string(),
            text_muted: "#98a2b3".to_string(),
            accent: "#2563eb".to_string(), // Azul cobalto etéreo
            border: "#d0d5dd".to_string(),
            success: "#12b76a".to_string(),
            warning: "#f79009".to_string(),
            danger: "#f04438".to_string(),
        }
    }

    /// Tema hermético alquímico: Arcana
    pub fn arcana() -> Self {
        Self {
            name: "Arcana".to_string(),
            description: "A Sabedoria Oculta — Mistérios herméticos, ametista mística e alquimia da mente".to_string(),
            background: "#181424".to_string(),
            sidebar_bg: "#100d19".to_string(),
            surface: "#261f38".to_string(),
            surface_hover: "#362c4f".to_string(),
            surface_active: "#483b69".to_string(),
            text_primary: "#f5f0ff".to_string(),
            text_secondary: "#cbbce6".to_string(),
            text_muted: "#7c6f96".to_string(),
            accent: "#c084fc".to_string(), // Púrpura ametista mágica
            border: "#342a4d".to_string(),
            success: "#4ade80".to_string(),
            warning: "#facc15".to_string(),
            danger: "#fb7185".to_string(),
        }
    }

    /// Tema ártico límpido: Boreas
    pub fn boreas() -> Self {
        Self {
            name: "Boreas".to_string(),
            description: "O Vento Boreal — Serenidade gélida do norte supremo e contemplação límpida".to_string(),
            background: "#242933".to_string(),
            sidebar_bg: "#1b1f27".to_string(),
            surface: "#2e3440".to_string(),
            surface_hover: "#3b4252".to_string(),
            surface_active: "#434c5e".to_string(),
            text_primary: "#eceff4".to_string(),
            text_secondary: "#d8dee9".to_string(),
            text_muted: "#7b889b".to_string(),
            accent: "#88c0d0".to_string(), // Azul glacial polar
            border: "#3b4252".to_string(),
            success: "#a3be8c".to_string(),
            warning: "#ebcb8b".to_string(),
            danger: "#bf616a".to_string(),
        }
    }
}

/// Gerenciador de temas dinâmicos via TOML.
#[derive(Debug, Clone)]
pub struct ThemeManager {
    pub themes: Vec<ThemeColors>,
    pub active_index: usize,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeManager {
    /// Inicializa o gerenciador com a coleção de temas filosóficos/místicos integrados.
    pub fn new() -> Self {
        Self {
            themes: vec![
                ThemeColors::abyss(),
                ThemeColors::aether(),
                ThemeColors::arcana(),
                ThemeColors::boreas(),
            ],
            active_index: 0,
        }
    }

    /// Retorna uma referência ao tema ativo no momento.
    pub fn active_theme(&self) -> &ThemeColors {
        self.themes.get(self.active_index).unwrap_or(&self.themes[0])
    }

    /// Alterna para o próximo tema em ciclo ordenado (Ctrl+Shift+T).
    pub fn cycle_next(&mut self) -> &ThemeColors {
        self.active_index = (self.active_index + 1) % self.themes.len();
        self.active_theme()
    }

    /// Seleciona um tema pelo nome (case-insensitive).
    pub fn select_by_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self
            .themes
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name))
        {
            self.active_index = pos;
            true
        } else {
            false
        }
    }

    /// Carrega e registra um tema a partir de uma string no formato TOML.
    pub fn load_from_toml(&mut self, toml_str: &str) -> Result<ThemeColors, String> {
        let parsed: ThemeColors = toml::from_str(toml_str).map_err(|e| format!("Erro no TOML: {}", e))?;

        if let Some(pos) = self.themes.iter().position(|t| t.name == parsed.name) {
            self.themes[pos] = parsed.clone();
            self.active_index = pos;
        } else {
            self.themes.push(parsed.clone());
            self.active_index = self.themes.len().saturating_sub(1);
        }

        Ok(parsed)
    }

    /// Exporta o tema atual para o formato TOML.
    pub fn export_current_to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self.active_theme()).map_err(|e| format!("Erro ao serializar: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mystical_themes_initialization_and_cycling() {
        let mut mgr = ThemeManager::new();
        assert_eq!(mgr.active_theme().name, "Abyss");

        // Cicla com Ctrl+Shift+T
        mgr.cycle_next();
        assert_eq!(mgr.active_theme().name, "Aether");

        mgr.cycle_next();
        assert_eq!(mgr.active_theme().name, "Arcana");

        mgr.cycle_next();
        assert_eq!(mgr.active_theme().name, "Boreas");

        // Volta ao início
        mgr.cycle_next();
        assert_eq!(mgr.active_theme().name, "Abyss");
    }

    #[test]
    fn test_theme_toml_roundtrip() {
        let mut mgr = ThemeManager::new();
        let toml_output = mgr.export_current_to_toml();
        assert!(toml_output.is_ok());

        if let Ok(toml_str) = toml_output {
            assert!(toml_str.contains("Abyss"));
            assert!(toml_str.contains("background"));

            let loaded = mgr.load_from_toml(&toml_str);
            assert!(loaded.is_ok());
            if let Ok(theme) = loaded {
                assert_eq!(theme.name, "Abyss");
            }
        }
    }

    #[test]
    fn test_select_by_mystical_name() {
        let mut mgr = ThemeManager::new();
        assert!(mgr.select_by_name("aether"));
        assert_eq!(mgr.active_theme().name, "Aether");
        assert_eq!(mgr.active_theme().accent, "#2563eb");

        assert!(mgr.select_by_name("ARCANA"));
        assert_eq!(mgr.active_theme().name, "Arcana");
        assert_eq!(mgr.active_theme().accent, "#c084fc");

        assert!(!mgr.select_by_name("Inexistente"));
    }
}
