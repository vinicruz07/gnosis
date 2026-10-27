//! Gerenciador do Quick Switcher (Ctrl+O) e da Paleta de Comandos (Ctrl+P).

use crate::fuzzy::fuzzy_match;
use serde::{Deserialize, Serialize};

/// Categoria do item listado no modal flutuante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickItemType {
    Note,
    Command,
    Tag,
}

impl QuickItemType {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Note => "📄",
            Self::Command => "⚡",
            Self::Tag => "🏷️",
        }
    }
}

/// Item catalogado para busca no Quick Switcher ou Paleta de Comandos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub item_type: QuickItemType,
    pub action_id: String,
}

/// Modo de operação do modal suspenso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum QuickMode {
    #[default]
    Files,
    Commands,
}

/// Orquestrador do Quick Switcher e Paleta de Comandos.
#[derive(Debug, Clone)]
pub struct QuickSwitcherManager {
    pub mode: QuickMode,
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub all_items: Vec<QuickItem>,
    pub filtered_items: Vec<QuickItem>,
}

impl Default for QuickSwitcherManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickSwitcherManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            mode: QuickMode::Files,
            is_open: false,
            query: String::new(),
            selected_index: 0,
            all_items: Vec::new(),
            filtered_items: Vec::new(),
        };

        mgr.register_builtin_commands();
        mgr
    }

    /// Registra os comandos globais integrados do Vault.
    pub fn register_builtin_commands(&mut self) {
        let builtins = [
            ("cmd_graph", "Abrir Grafo de Conhecimento", "Visualizar trama de notas 2D e física", "Ctrl+G"),
            ("cmd_toggle_sidebar", "Alternar Barra Lateral", "Mostrar ou recolher árvore de arquivos", "Ctrl+B"),
            ("cmd_toggle_split", "Alternar Divisão de Tela", "Ativar visualização lado a lado", "Ctrl+\\"),
            ("cmd_new_note", "Criar Nova Nota", "Adicionar nota vazia no Vault", "Ctrl+N"),
            ("cmd_toggle_theme", "Alternar Tema Visual", "Mudar entre Dark, Light, Dracula e Nord", "Ctrl+Shift+T"),
            ("cmd_sync_now", "Sincronizar Vault Agora", "Conectar aos pares P2P da sala", "Ctrl+S"),
            ("cmd_pick_sticker", "Escolher Ícone / Sticker da Nota", "Adicionar emoji ou sticker no frontmatter da nota atual", "Ctrl+Shift+E"),
            ("cmd_history_back", "Histórico: Voltar", "Retornar à nota visitada anteriormente", "Alt+Left"),
            ("cmd_history_forward", "Histórico: Avançar", "Avançar na pilha de histórico", "Alt+Right"),
        ];


        for (id, title, desc, shortcut) in builtins {
            self.all_items.push(QuickItem {
                id: id.to_string(),
                title: title.to_string(),
                subtitle: format!("{} • {}", shortcut, desc),
                item_type: QuickItemType::Command,
                action_id: id.to_string(),
            });
        }
    }

    /// Registra uma nota do Vault no catálogo de busca.
    pub fn register_note(&mut self, path: &str, title: &str) {
        let subtitle = path.rsplit('/').nth(1).unwrap_or("Raiz do Vault");

        // Evita duplicatas pelo ID (caminho)
        if !self.all_items.iter().any(|i| i.id == path) {
            self.all_items.push(QuickItem {
                id: path.to_string(),
                title: title.to_string(),
                subtitle: subtitle.to_string(),
                item_type: QuickItemType::Note,
                action_id: path.to_string(),
            });
        }
    }

    /// Abre o modal no modo especificado (Arquivos ou Comandos).
    pub fn open(&mut self, mode: QuickMode) {
        self.mode = mode;
        self.is_open = true;
        self.query.clear();
        self.selected_index = 0;
        self.update_filter();
    }

    /// Fecha o modal suspenso.
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    /// Atualiza o texto da busca difusa.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.update_filter();
    }

    /// Navega para o próximo item da lista.
    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_items.len();
        }
    }

    /// Navega para o item anterior da lista.
    pub fn select_prev(&mut self) {
        if !self.filtered_items.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_items.len().saturating_sub(1);
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Retorna o item atualmente selecionado.
    pub fn selected_item(&self) -> Option<&QuickItem> {
        self.filtered_items.get(self.selected_index)
    }

    /// Atualiza a lista filtrada com base no modo atual e na query difusa.
    fn update_filter(&mut self) {
        let target_type = match self.mode {
            QuickMode::Files => QuickItemType::Note,
            QuickMode::Commands => QuickItemType::Command,
        };

        let mut scored_items: Vec<(i32, QuickItem)> = Vec::new();

        for item in &self.all_items {
            if item.item_type != target_type {
                continue;
            }

            if self.query.is_empty() {
                scored_items.push((0, item.clone()));
            } else {
                // Tenta casar no título da nota ou comando
                let title_match = fuzzy_match(&self.query, &item.title);
                // Tenta casar no subtítulo (caminho da pasta ou atalho)
                let sub_match = fuzzy_match(&self.query, &item.subtitle);

                match (title_match, sub_match) {
                    (Some(t), Some(s)) => {
                        let best_score = t.score.max(s.score);
                        scored_items.push((best_score, item.clone()));
                    }
                    (Some(t), None) => {
                        scored_items.push((t.score, item.clone()));
                    }
                    (None, Some(s)) => {
                        scored_items.push((s.score, item.clone()));
                    }
                    (None, None) => {}
                }
            }
        }

        // Ordena por pontuação decrescente de relevância
        scored_items.sort_by(|a, b| b.0.cmp(&a.0));

        self.filtered_items = scored_items.into_iter().map(|(_, item)| item).collect();

        // Garante que o selected_index permaneça em limites válidos
        if self.selected_index >= self.filtered_items.len() {
            self.selected_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_switcher_modes_and_filtering() {
        let mut mgr = QuickSwitcherManager::new();

        mgr.register_note("00_Inbox/Ideias Rápidas.md", "Ideias Rápidas");
        mgr.register_note("01_Projetos/Rust/Gnosis Core.md", "Gnosis Core");
        mgr.register_note("02_Estudos/Grafos.md", "Grafos e Física");

        // Abre modo de notas (Ctrl+O)
        mgr.open(QuickMode::Files);
        assert_eq!(mgr.filtered_items.len(), 3);

        // Busca por "gno"
        mgr.set_query("gno");
        assert_eq!(mgr.filtered_items.len(), 1);
        assert_eq!(mgr.filtered_items[0].title, "Gnosis Core");

        // Navegação
        mgr.open(QuickMode::Commands);
        assert!(!mgr.filtered_items.is_empty());
        assert_eq!(mgr.selected_index, 0);

        mgr.select_next();
        assert_eq!(mgr.selected_index, 1);

        mgr.select_prev();
        assert_eq!(mgr.selected_index, 0);

        // Fecha
        mgr.close();
        assert!(!mgr.is_open);
    }
}
