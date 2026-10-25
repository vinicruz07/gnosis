//! Gerenciamento de estado da interface do usuário (UI) e árvore de arquivos do Vault.

use std::collections::HashSet;

/// Representação interna de um nó na árvore de arquivos do Vault.
#[derive(Debug, Clone, PartialEq)]
pub struct FileTreeEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_folder: bool,
    pub parent_path: Option<String>,
}

/// Item projetado para consumo do modelo de renderização da UI Slint.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeItemModel {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_folder: bool,
    pub is_expanded: bool,
    pub indent_level: i32,
    pub is_active: bool,
}

/// Estado global da interface da aplicação.
#[derive(Debug, Clone)]
pub struct AppState {
    pub vault_name: String,
    pub vault_path: String,
    pub active_note_path: Option<String>,
    pub active_note_title: String,
    pub active_note_content: String,
    pub sidebar_collapsed: bool,
    pub sync_status: String,
    pub entries: Vec<FileTreeEntry>,
    pub expanded_folders: HashSet<String>,
}

impl AppState {
    /// Inicializa um novo estado da aplicação com os valores padrões.
    pub fn new(vault_name: impl Into<String>, vault_path: impl Into<String>) -> Self {
        Self {
            vault_name: vault_name.into(),
            vault_path: vault_path.into(),
            active_note_path: None,
            active_note_title: "Boas-vindas ao Gnosis".to_string(),
            active_note_content: "# Bem-vindo ao Gnosis\n\nRecompilação de alto desempenho do Obsidian em Rust puro.".to_string(),
            sidebar_collapsed: false,
            sync_status: "Sincronizado (Local)".to_string(),
            entries: Vec::new(),
            expanded_folders: HashSet::new(),
        }
    }

    /// Alterna a visibilidade da barra lateral (expandida / recolhida).
    pub fn toggle_sidebar(&mut self) -> bool {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.sidebar_collapsed
    }

    /// Alterna o estado de expansão de uma pasta na árvore de arquivos.
    pub fn toggle_folder(&mut self, folder_path: &str) {
        if self.expanded_folders.contains(folder_path) {
            self.expanded_folders.remove(folder_path);
        } else {
            self.expanded_folders.insert(folder_path.to_string());
        }
    }

    /// Seleciona uma nota para exibição ativa e calcula métricas.
    pub fn select_note(&mut self, path: &str, content: &str) {
        self.active_note_path = Some(path.to_string());
        self.active_note_content = content.to_string();

        // Extrai o nome da nota a partir do caminho
        let title = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".md");
        self.active_note_title = title.to_string();
    }

    /// Adiciona um arquivo ou pasta ao catálogo da árvore.
    pub fn add_entry(&mut self, entry: FileTreeEntry) {
        if !self.entries.iter().any(|e| e.path == entry.path) {
            self.entries.push(entry);
        }
    }

    /// Popula a lista de entradas a partir de caminhos relativos de arquivos (ex: "Notas/Estudos/Rust.md").
    pub fn populate_from_paths<I, S>(&mut self, paths: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut folders_seen: HashSet<String> = HashSet::new();

        for p_ref in paths {
            let path_str = p_ref.as_ref();
            let segments: Vec<&str> = path_str.split('/').collect();

            // Cadastra pastas intermediárias
            let mut curr_folder = String::new();
            for i in 0..segments.len().saturating_sub(1) {
                let seg = segments[i];
                let parent = if curr_folder.is_empty() {
                    None
                } else {
                    Some(curr_folder.clone())
                };

                if !curr_folder.is_empty() {
                    curr_folder.push('/');
                }
                curr_folder.push_str(seg);

                if folders_seen.insert(curr_folder.clone()) {
                    self.add_entry(FileTreeEntry {
                        id: curr_folder.clone(),
                        name: seg.to_string(),
                        path: curr_folder.clone(),
                        is_folder: true,
                        parent_path: parent,
                    });
                }
            }

            // Cadastra a nota/arquivo final
            let file_name = segments.last().copied().unwrap_or(path_str);
            let parent_path = if segments.len() > 1 {
                Some(segments[0..segments.len() - 1].join("/"))
            } else {
                None
            };

            self.add_entry(FileTreeEntry {
                id: path_str.to_string(),
                name: file_name.trim_end_matches(".md").to_string(),
                path: path_str.to_string(),
                is_folder: false,
                parent_path,
            });
        }
    }

    /// Gera a lista achatada de itens visíveis para renderização no Slint,
    /// respeitando as pastas expandidas e níveis de indentação.
    pub fn visible_tree_items(&self) -> Vec<TreeItemModel> {
        let mut result = Vec::new();

        // Função recursiva para visitar a árvore mantendo hierarquia ordenada
        fn visit(
            state: &AppState,
            parent: Option<&str>,
            level: i32,
            out: &mut Vec<TreeItemModel>,
        ) {
            // Separa filhos imediatos do parent
            let mut children: Vec<&FileTreeEntry> = state
                .entries
                .iter()
                .filter(|e| e.parent_path.as_deref() == parent)
                .collect();

            // Ordena: pastas primeiro, depois notas por nome alfabético
            children.sort_by(|a, b| {
                b.is_folder
                    .cmp(&a.is_folder)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });

            for child in children {
                let is_expanded = state.expanded_folders.contains(&child.path);
                let is_active = state.active_note_path.as_deref() == Some(&child.path);

                out.push(TreeItemModel {
                    id: child.id.clone(),
                    name: child.name.clone(),
                    path: child.path.clone(),
                    is_folder: child.is_folder,
                    is_expanded,
                    indent_level: level,
                    is_active,
                });

                // Se for pasta e estiver expandida, visita os filhos recursivamente
                if child.is_folder && is_expanded {
                    visit(state, Some(&child.path), level + 1, out);
                }
            }
        }

        visit(self, None, 0, &mut result);
        result
    }

    /// Contagem precisa de palavras do conteúdo ativo.
    pub fn word_count(&self) -> usize {
        self.active_note_content
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .count()
    }

    /// Contagem total de caracteres do conteúdo ativo.
    pub fn char_count(&self) -> usize {
        self.active_note_content.chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_metrics_and_sidebar_toggle() {
        let mut state = AppState::new("Vault de Teste", "/caminho/vault");
        assert!(!state.sidebar_collapsed);

        let collapsed = state.toggle_sidebar();
        assert!(collapsed);
        assert!(state.sidebar_collapsed);

        let reexpanded = state.toggle_sidebar();
        assert!(!reexpanded);
        assert!(!state.sidebar_collapsed);

        state.select_note("Inbox/Ideias.md", "Uma duas três quatro palavras.");
        assert_eq!(state.active_note_title, "Ideias");
        assert_eq!(state.word_count(), 5);
        assert_eq!(state.char_count(), 31);
    }

    #[test]
    fn test_file_tree_hierarchy_and_folder_expansion() {
        let mut state = AppState::new("Vault", "/vault");

        let paths = vec![
            "Projetos/Rust/Gnosis.md",
            "Projetos/Web/App.md",
            "Diario.md",
        ];

        state.populate_from_paths(paths);

        // Inicialmente, nenhuma pasta está expandida: devem aparecer apenas 'Diario' e 'Projetos'
        let initial_items = state.visible_tree_items();
        assert_eq!(initial_items.len(), 2);
        assert_eq!(initial_items[0].name, "Projetos");
        assert!(initial_items[0].is_folder);
        assert_eq!(initial_items[1].name, "Diario");
        assert!(!initial_items[1].is_folder);

        // Expande 'Projetos'
        state.toggle_folder("Projetos");
        let expanded_items = state.visible_tree_items();
        // Agora deve listar: Projetos (pasta), Rust (subpasta fechada), Web (subpasta fechada), Diario
        assert_eq!(expanded_items.len(), 4);

        // Expande 'Projetos/Rust'
        state.toggle_folder("Projetos/Rust");
        let rust_expanded = state.visible_tree_items();
        // Deve listar também 'Gnosis' com indent_level = 2
        let gnosis_node = rust_expanded.iter().find(|i| i.name == "Gnosis");
        assert!(gnosis_node.is_some());
        if let Some(node) = gnosis_node {
            assert_eq!(node.indent_level, 2);
            assert_eq!(node.path, "Projetos/Rust/Gnosis.md");
        }
    }
}
