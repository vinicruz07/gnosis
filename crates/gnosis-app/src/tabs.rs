//! Gerenciador de abas múltiplas, split views (divisão de tela) e histórico de navegação.

use serde::{Deserialize, Serialize};

/// Informações de uma aba individual aberta no editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabItem {
    pub id: String,
    pub path: String,
    pub title: String,
    pub is_pinned: bool,
    pub is_modified: bool,
    pub cursor_position: usize,
    pub scroll_offset: f32,
    pub icon: Option<String>,
}

impl TabItem {
    pub fn new(path: impl Into<String>, title: impl Into<String>) -> Self {
        let p = path.into();
        let t = title.into();
        Self {
            id: p.clone(),
            path: p,
            title: t,
            is_pinned: false,
            is_modified: false,
            cursor_position: 0,
            scroll_offset: 0.0,
            icon: None,
        }
    }

    /// Anexa um ícone ou sticker personalizado à aba.
    pub fn with_icon(mut self, icon: Option<impl Into<String>>) -> Self {
        self.icon = icon.map(|i| i.into());
        self
    }
}


/// Modo de divisão de tela entre painéis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SplitDirection {
    #[default]
    Single,
    Horizontal,
    Vertical,
}

/// Painel de edição que contém uma coleção de abas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditorPane {
    pub id: usize,
    pub tabs: Vec<TabItem>,
    pub active_tab_index: usize,
}

impl EditorPane {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            active_tab_index: 0,
        }
    }

    /// Retorna a aba ativa no painel, se houver.
    pub fn active_tab(&self) -> Option<&TabItem> {
        self.tabs.get(self.active_tab_index)
    }

    /// Retorna uma referência mutável à aba ativa.
    pub fn active_tab_mut(&mut self) -> Option<&mut TabItem> {
        self.tabs.get_mut(self.active_tab_index)
    }

    /// Abre uma nota no painel com ícone opcional. Se já estiver aberta, atualiza o ícone e ativa.
    pub fn open_tab_with_icon(&mut self, path: &str, title: &str, icon: Option<String>) -> usize {
        if let Some(pos) = self.tabs.iter().position(|t| t.path == path) {
            if icon.is_some() {
                self.tabs[pos].icon = icon;
            }
            self.active_tab_index = pos;
            return pos;
        }

        let new_tab = TabItem::new(path, title).with_icon(icon);
        self.tabs.push(new_tab);
        self.active_tab_index = self.tabs.len().saturating_sub(1);
        self.active_tab_index
    }

    /// Abre uma nota no painel. Se já estiver aberta, apenas a ativa.
    pub fn open_tab(&mut self, path: &str, title: &str) -> usize {
        self.open_tab_with_icon(path, title, None)
    }


    /// Fecha a aba no índice especificado e reajusta a aba ativa com segurança.
    pub fn close_tab(&mut self, index: usize) -> Option<TabItem> {
        if index >= self.tabs.len() {
            return None;
        }

        let removed = self.tabs.remove(index);

        if self.tabs.is_empty() {
            self.active_tab_index = 0;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len().saturating_sub(1);
        } else if index < self.active_tab_index {
            self.active_tab_index = self.active_tab_index.saturating_sub(1);
        }

        Some(removed)
    }

    /// Seleciona a aba no índice indicado.
    pub fn select_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            true
        } else {
            false
        }
    }
}

/// Histórico bidirecional de navegação (estilo voltar/avançar do navegador).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NavigationHistory {
    back_stack: Vec<String>,
    forward_stack: Vec<String>,
    current: Option<String>,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra uma nova navegação para `path`.
    pub fn push(&mut self, path: &str) {
        if let Some(curr) = self.current.take() {
            if curr != path {
                self.back_stack.push(curr);
            }
        }
        self.current = Some(path.to_string());
        // Ao navegar para um novo local, a pilha para a frente é resetada
        self.forward_stack.clear();
    }

    /// Retorna à nota anterior na pilha de histórico.
    pub fn go_back(&mut self) -> Option<String> {
        let prev = self.back_stack.pop()?;
        if let Some(curr) = self.current.take() {
            self.forward_stack.push(curr);
        }
        self.current = Some(prev.clone());
        Some(prev)
    }

    /// Avança para a próxima nota na pilha de histórico.
    pub fn go_forward(&mut self) -> Option<String> {
        let next = self.forward_stack.pop()?;
        if let Some(curr) = self.current.take() {
            self.back_stack.push(curr);
        }
        self.current = Some(next.clone());
        Some(next)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }
}

/// Orquestrador de múltiplas abas, divisões de tela e histórico.
#[derive(Debug, Clone)]
pub struct TabManager {
    pub panes: Vec<EditorPane>,
    pub active_pane_index: usize,
    pub split_direction: SplitDirection,
    pub history: NavigationHistory,
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TabManager {
    /// Inicializa o gerenciador com um painel principal vazio.
    pub fn new() -> Self {
        let mut initial_pane = EditorPane::new(0);
        initial_pane.open_tab("Boas-vindas.md", "Boas-vindas");

        Self {
            panes: vec![initial_pane],
            active_pane_index: 0,
            split_direction: SplitDirection::Single,
            history: NavigationHistory::new(),
        }
    }

    /// Retorna referência ao painel em foco no momento.
    pub fn active_pane(&self) -> &EditorPane {
        self.panes
            .get(self.active_pane_index)
            .unwrap_or(&self.panes[0])
    }

    /// Retorna referência mutável ao painel em foco.
    pub fn active_pane_mut(&mut self) -> &mut EditorPane {
        if self.active_pane_index >= self.panes.len() {
            self.active_pane_index = 0;
        }
        &mut self.panes[self.active_pane_index]
    }

    /// Abre uma nota no painel ativo com ícone opcional e registra no histórico de navegação.
    pub fn open_note_with_icon(&mut self, path: &str, title: &str, icon: Option<String>) {
        self.history.push(path);
        let pane = self.active_pane_mut();
        pane.open_tab_with_icon(path, title, icon);
    }

    /// Abre uma nota no painel ativo e registra no histórico de navegação.
    pub fn open_note(&mut self, path: &str, title: &str) {
        self.open_note_with_icon(path, title, None);
    }


    /// Fecha a aba ativa do painel ativo.
    pub fn close_active_tab(&mut self) -> Option<TabItem> {
        let idx = self.active_pane().active_tab_index;
        self.active_pane_mut().close_tab(idx)
    }

    /// Alterna ou define a divisão de tela entre painéis.
    pub fn set_split(&mut self, direction: SplitDirection) {
        self.split_direction = direction;

        if direction != SplitDirection::Single && self.panes.len() < 2 {
            // Cria o segundo painel para suportar a divisão de tela
            let mut second_pane = EditorPane::new(1);
            if let Some(active_tab) = self.active_pane().active_tab() {
                second_pane.open_tab(&active_tab.path, &active_tab.title);
            } else {
                second_pane.open_tab("Divisão.md", "Divisão");
            }
            self.panes.push(second_pane);
        } else if direction == SplitDirection::Single && self.panes.len() > 1 {
            // Unifica abas de volta ao primeiro painel
            if let Some(second_pane) = self.panes.pop() {
                for tab in second_pane.tabs {
                    if !self.panes[0].tabs.iter().any(|t| t.path == tab.path) {
                        self.panes[0].tabs.push(tab);
                    }
                }
            }
            self.active_pane_index = 0;
        }
    }

    /// Retorna à nota anterior pelo histórico.
    pub fn navigate_back(&mut self) -> Option<String> {
        let path = self.history.go_back()?;
        let title = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .trim_end_matches(".md");
        self.active_pane_mut().open_tab(&path, title);
        Some(path)
    }

    /// Avança à próxima nota pelo histórico.
    pub fn navigate_forward(&mut self) -> Option<String> {
        let path = self.history.go_forward()?;
        let title = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .trim_end_matches(".md");
        self.active_pane_mut().open_tab(&path, title);
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_multiple_tabs_and_switch_preserves_state() {
        let mut pane = EditorPane::new(0);

        // Abre 10 abas
        for i in 1..=10 {
            let path = format!("Nota_{}.md", i);
            let title = format!("Nota {}", i);
            pane.open_tab(&path, &title);
        }

        assert_eq!(pane.tabs.len(), 10);
        assert_eq!(pane.active_tab_index, 9); // Última aberta é a ativa

        // Atualiza a posição do cursor na aba 10
        if let Some(tab) = pane.active_tab_mut() {
            tab.cursor_position = 42;
            tab.scroll_offset = 120.5;
        }

        // Troca para a aba 3
        let switched = pane.select_tab(2);
        assert!(switched);
        assert_eq!(pane.active_tab().map(|t| t.title.as_str()), Some("Nota 3"));

        // Volta para a aba 10 e verifica se o cursor foi preservado
        pane.select_tab(9);
        let tab10 = pane.active_tab().unwrap_or(&pane.tabs[0]);
        assert_eq!(tab10.cursor_position, 42);
        assert_eq!(tab10.scroll_offset, 120.5);
    }

    #[test]
    fn test_closing_tab_adjusts_active_index_safely() {
        let mut pane = EditorPane::new(0);
        pane.open_tab("A.md", "A");
        pane.open_tab("B.md", "B");
        pane.open_tab("C.md", "C");

        pane.select_tab(1); // B ativa
        let closed = pane.close_tab(1);
        assert_eq!(closed.map(|t| t.title), Some("B".to_string()));
        assert_eq!(pane.tabs.len(), 2);
        // Ao fechar B, a aba ativa deve ser ajustada para C
        assert_eq!(pane.active_tab().map(|t| t.title.as_str()), Some("C"));

        // Fecha a última aba (C)
        let closed_c = pane.close_tab(1);
        assert_eq!(closed_c.map(|t| t.title), Some("C".to_string()));
        // Resta apenas A no índice 0
        assert_eq!(pane.active_tab_index, 0);
        assert_eq!(pane.active_tab().map(|t| t.title.as_str()), Some("A"));
    }

    #[test]
    fn test_navigation_history_back_and_forward() {
        let mut manager = TabManager::new();

        manager.open_note("Nota_1.md", "Nota 1");
        manager.open_note("Nota_2.md", "Nota 2");
        manager.open_note("Nota_3.md", "Nota 3");

        assert!(manager.history.can_go_back());
        assert!(!manager.history.can_go_forward());

        // Volta para Nota 2
        let back1 = manager.navigate_back();
        assert_eq!(back1.as_deref(), Some("Nota_2.md"));
        assert!(manager.history.can_go_forward());

        // Volta para Nota 1
        let back2 = manager.navigate_back();
        assert_eq!(back2.as_deref(), Some("Nota_1.md"));

        // Avança para Nota 2
        let fwd1 = manager.navigate_forward();
        assert_eq!(fwd1.as_deref(), Some("Nota_2.md"));

        // Avança para Nota 3
        let fwd2 = manager.navigate_forward();
        assert_eq!(fwd2.as_deref(), Some("Nota_3.md"));
        assert!(!manager.history.can_go_forward());
    }

    #[test]
    fn test_split_views_horizontal_and_unification() {
        let mut manager = TabManager::new();
        manager.open_note("A.md", "A");

        assert_eq!(manager.panes.len(), 1);

        // Divide a tela horizontalmente
        manager.set_split(SplitDirection::Horizontal);
        assert_eq!(manager.panes.len(), 2);
        assert_eq!(manager.split_direction, SplitDirection::Horizontal);

        // Abre uma segunda nota no painel da direita
        manager.active_pane_index = 1;
        manager.open_note("B.md", "B");
        assert_eq!(manager.panes[1].active_tab().map(|t| t.title.as_str()), Some("B"));

        // Fecha a divisão unificando
        manager.set_split(SplitDirection::Single);
        assert_eq!(manager.panes.len(), 1);
        assert_eq!(manager.split_direction, SplitDirection::Single);
        // As abas devem ter sido consolidadas
        assert!(manager.panes[0].tabs.iter().any(|t| t.title == "B"));
    }

    #[test]
    fn test_tab_with_sticker_icon() {
        let mut manager = TabManager::new();
        manager.open_note_with_icon("Doc.md", "Documentação", Some("📚".to_string()));

        let active = manager.active_pane().active_tab();
        assert!(active.is_some());
        if let Some(tab) = active {
            assert_eq!(tab.title, "Documentação");
            assert_eq!(tab.icon.as_deref(), Some("📚"));
        }
    }
}

