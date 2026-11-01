//! Controlador de preferências e janela modal de configurações do Gnosis.
//!
//! Implementação conforme as diretrizes:
//! - 100% Rust Seguro (`#![deny(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Gerenciamento de abas: Geral, Aparência, Editor e Sobre.

use serde::{Deserialize, Serialize};

/// Abas de navegação do modal de configurações.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingsTab {
    /// Configurações gerais da aplicação e do cofre.
    General,
    /// Customização visual (temas, densidade da interface).
    Appearance,
    /// Preferências de digitação e do editor (Live Preview, fontes, quebras de linha).
    Editor,
    /// Informações sobre o Gnosis, versão e motor gráfico legado.
    About,
}

impl SettingsTab {
    /// Converte o enum para índice numérico de 0 a 3 para interface Slint.
    pub fn to_index(self) -> i32 {
        match self {
            Self::General => 0,
            Self::Appearance => 1,
            Self::Editor => 2,
            Self::About => 3,
        }
    }

    /// Obtém a aba correspondente a partir de um índice numérico.
    /// Retorna `General` com segurança caso o índice seja desconhecido.
    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::General,
            1 => Self::Appearance,
            2 => Self::Editor,
            3 => Self::About,
            _ => Self::General,
        }
    }

    /// Rótulo legível para exibição em menus.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::General => "Geral",
            Self::Appearance => "Aparência",
            Self::Editor => "Editor",
            Self::About => "Sobre o Gnosis",
        }
    }

    /// Ícone representativo da seção.
    pub fn icon(self) -> &'static str {
        match self {
            Self::General => "⚙️",
            Self::Appearance => "🎨",
            Self::Editor => "✏️",
            Self::About => "ℹ️",
        }
    }
}

/// Preferências do usuário persistíveis e configuráveis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSettings {
    /// Tamanho de fonte padrão do editor em pixels (clamp 10..=32).
    pub font_size: u32,
    /// Quebra automática de linha no editor de texto.
    pub line_wrapping: bool,
    /// Largura da tabulação em espaços (clamp 2..=8).
    pub tab_size: u32,
    /// Atraso de salvamento automático em milissegundos.
    pub auto_save_delay_ms: u64,
    /// Iniciar notas preferencialmente no modo Live Preview (WYSIWYG).
    pub default_live_preview: bool,
    /// Identificador do tema ativo (ex: "dark", "light", "dracula").
    pub theme_id: String,
    /// Modo de interface compacta para telas menores / densas.
    pub compact_mode: bool,
    /// Informações do motor de renderização.
    pub engine_info: String,
    /// Versão da aplicação.
    pub version: String,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            font_size: 15,
            line_wrapping: true,
            tab_size: 4,
            auto_save_delay_ms: 1000,
            default_live_preview: true,
            theme_id: "dark".to_string(),
            compact_mode: false,
            engine_info: "Slint FemtoVG / Software Renderer (Hardware 2015- 60FPS)".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl UserSettings {
    /// Atualiza o tamanho de fonte com contenção segura entre 10px e 32px.
    pub fn set_font_size(&mut self, size: u32) {
        self.font_size = size.clamp(10, 32);
    }

    /// Atualiza o tamanho de tabulação com contenção segura entre 2 e 8 espaços.
    pub fn set_tab_size(&mut self, size: u32) {
        self.tab_size = size.clamp(2, 8);
    }
}

/// Controlador responsável pelo estado, visibilidade e manipulação do modal de configurações.
#[derive(Debug, Clone, Default)]
pub struct SettingsController {
    /// Indica se o modal de preferências está visível na tela.
    is_open: bool,
    /// Aba atualmente selecionada no modal.
    active_tab: SettingsTab,
    /// Configurações do usuário.
    settings: UserSettings,
}

impl SettingsController {
    /// Cria uma nova instância com valores padrão e modal fechado.
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: SettingsTab::General,
            settings: UserSettings::default(),
        }
    }

    /// Verifica se o modal está aberto.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Abre o modal de configurações na aba ativa atual.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Fecha o modal de configurações.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Alterna a visibilidade do modal de configurações.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Obtém a aba ativa no momento.
    pub fn active_tab(&self) -> SettingsTab {
        self.active_tab
    }

    /// Obtém o índice da aba ativa para a camada Slint.
    pub fn active_tab_index(&self) -> i32 {
        self.active_tab.to_index()
    }

    /// Define a aba ativa diretamente por enum.
    pub fn select_tab(&mut self, tab: SettingsTab) {
        self.active_tab = tab;
    }

    /// Define a aba ativa por índice com mapeamento seguro e sem pânico.
    pub fn select_tab_by_index(&mut self, index: i32) {
        self.active_tab = SettingsTab::from_index(index);
    }

    /// Referência imutável para as configurações do usuário.
    pub fn settings(&self) -> &UserSettings {
        &self.settings
    }

    /// Referência mutável para as configurações do usuário.
    pub fn settings_mut(&mut self) -> &mut UserSettings {
        &mut self.settings
    }
}

impl Default for SettingsTab {
    fn default() -> Self {
        Self::General
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_tab_indices_and_fallbacks() {
        assert_eq!(SettingsTab::General.to_index(), 0);
        assert_eq!(SettingsTab::Appearance.to_index(), 1);
        assert_eq!(SettingsTab::Editor.to_index(), 2);
        assert_eq!(SettingsTab::About.to_index(), 3);

        assert_eq!(SettingsTab::from_index(0), SettingsTab::General);
        assert_eq!(SettingsTab::from_index(1), SettingsTab::Appearance);
        assert_eq!(SettingsTab::from_index(2), SettingsTab::Editor);
        assert_eq!(SettingsTab::from_index(3), SettingsTab::About);

        // Fallback seguro para índices fora do intervalo
        assert_eq!(SettingsTab::from_index(-1), SettingsTab::General);
        assert_eq!(SettingsTab::from_index(42), SettingsTab::General);
    }

    #[test]
    fn test_modal_lifecycle_open_close_toggle() {
        let mut controller = SettingsController::new();
        assert!(!controller.is_open());

        controller.open();
        assert!(controller.is_open());

        controller.close();
        assert!(!controller.is_open());

        controller.toggle();
        assert!(controller.is_open());

        controller.toggle();
        assert!(!controller.is_open());
    }

    #[test]
    fn test_tab_selection_in_controller() {
        let mut controller = SettingsController::new();
        assert_eq!(controller.active_tab(), SettingsTab::General);
        assert_eq!(controller.active_tab_index(), 0);

        controller.select_tab(SettingsTab::Appearance);
        assert_eq!(controller.active_tab(), SettingsTab::Appearance);
        assert_eq!(controller.active_tab_index(), 1);

        controller.select_tab_by_index(2);
        assert_eq!(controller.active_tab(), SettingsTab::Editor);
        assert_eq!(controller.active_tab_index(), 2);

        controller.select_tab_by_index(3);
        assert_eq!(controller.active_tab(), SettingsTab::About);
        assert_eq!(controller.active_tab_index(), 3);

        // Índice inválido reverte para General com segurança
        controller.select_tab_by_index(99);
        assert_eq!(controller.active_tab(), SettingsTab::General);
    }

    #[test]
    fn test_user_settings_clamping() {
        let mut settings = UserSettings::default();
        assert_eq!(settings.font_size, 15);
        assert_eq!(settings.tab_size, 4);

        // Clamping de tamanho de fonte
        settings.set_font_size(5);
        assert_eq!(settings.font_size, 10); // mínimo 10
        settings.set_font_size(50);
        assert_eq!(settings.font_size, 32); // máximo 32
        settings.set_font_size(18);
        assert_eq!(settings.font_size, 18);

        // Clamping de tabulação
        settings.set_tab_size(0);
        assert_eq!(settings.tab_size, 2); // mínimo 2
        settings.set_tab_size(16);
        assert_eq!(settings.tab_size, 8); // máximo 8
        settings.set_tab_size(4);
        assert_eq!(settings.tab_size, 4);
    }

    #[test]
    fn test_tab_labels_and_icons() {
        assert_eq!(SettingsTab::General.display_name(), "Geral");
        assert_eq!(SettingsTab::Appearance.display_name(), "Aparência");
        assert_eq!(SettingsTab::Editor.display_name(), "Editor");
        assert_eq!(SettingsTab::About.display_name(), "Sobre o Gnosis");

        assert_eq!(SettingsTab::General.icon(), "⚙️");
        assert_eq!(SettingsTab::Appearance.icon(), "🎨");
        assert_eq!(SettingsTab::Editor.icon(), "✏️");
        assert_eq!(SettingsTab::About.icon(), "ℹ️");
    }

    #[test]
    fn test_settings_comprehensive_mutations_and_serialization() {
        let mut controller = SettingsController::new();
        
        // Mutação de propriedades através do controlador
        {
            let s = controller.settings_mut();
            s.set_font_size(20);
            s.line_wrapping = false;
            s.set_tab_size(6);
            s.default_live_preview = false;
            s.compact_mode = true;
            s.theme_id = "aether".to_string();
        }

        let s = controller.settings();
        assert_eq!(s.font_size, 20);
        assert!(!s.line_wrapping);
        assert_eq!(s.tab_size, 6);
        assert!(!s.default_live_preview);
        assert!(s.compact_mode);
        assert_eq!(s.theme_id, "aether");
        assert!(s.engine_info.contains("FemtoVG"));
        assert!(!s.version.is_empty());

        // Serialização e desserialização JSON seguras
        let json = serde_json::to_string(s).unwrap_or_default();
        assert!(!json.is_empty());
        let recovered: UserSettings = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(recovered, *s);
    }
}

