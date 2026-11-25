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
    pub icon: Option<String>,
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
    pub icon: String,
}

/// Estado global da interface da aplicação.
#[derive(Debug, Clone)]
pub struct AppState {
    pub vault_name: String,
    pub vault_path: String,
    pub active_note_path: Option<String>,
    pub active_note_title: String,
    pub active_note_icon: String,
    pub active_note_content: String,
    pub sidebar_collapsed: bool,
    pub sync_status: String,
    pub entries: Vec<FileTreeEntry>,
    pub expanded_folders: HashSet<String>,
    pub is_mobile: bool,
    pub mobile_drawer_open: bool,
    pub ime_bottom_inset: f32,
}

impl AppState {
    /// Inicializa um novo estado da aplicação com os valores padrões.
    pub fn new(vault_name: impl Into<String>, vault_path: impl Into<String>) -> Self {
        Self {
            vault_name: vault_name.into(),
            vault_path: vault_path.into(),
            active_note_path: None,
            active_note_title: "Boas-vindas ao Gnosis".to_string(),
            active_note_icon: String::new(),
            active_note_content: "# Bem-vindo ao Gnosis\n\nRecompilação de alto desempenho do Obsidian em Rust puro.".to_string(),
            sidebar_collapsed: false,
            sync_status: "Sincronizado (Local)".to_string(),
            entries: Vec::new(),
            expanded_folders: HashSet::new(),
            is_mobile: false,
            mobile_drawer_open: false,
            ime_bottom_inset: 0.0,
        }
    }

    /// Define o recuo inferior da janela causado pelo teclado virtual (IME) no mobile.
    pub fn set_ime_bottom_inset(&mut self, inset: f32) {
        self.ime_bottom_inset = inset.max(0.0);
    }

    /// Retorna o recuo inferior do teclado virtual em pixels.
    pub fn ime_bottom_inset(&self) -> f32 {
        self.ime_bottom_inset
    }

    /// Alterna a abertura da gaveta lateral em dispositivos móveis.
    pub fn toggle_mobile_drawer(&mut self) -> bool {
        self.mobile_drawer_open = !self.mobile_drawer_open;
        self.mobile_drawer_open
    }

    /// Fecha a gaveta lateral móvel.
    pub fn close_mobile_drawer(&mut self) {
        self.mobile_drawer_open = false;
    }

    /// Define o modo móvel ativando ou desativando layouts adaptativos.
    pub fn set_mobile(&mut self, is_mobile: bool) {
        self.is_mobile = is_mobile;
        if !is_mobile {
            self.mobile_drawer_open = false;
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

    /// Seleciona uma nota para exibição ativa, extrai metadados e calcula métricas.
    pub fn select_note(&mut self, path: &str, content: &str) {
        self.active_note_path = Some(path.to_string());
        self.active_note_content = content.to_string();

        let (fm_opt, _) = gnosis_markdown::parse_frontmatter(content);
        if let Some(fm) = fm_opt {
            if let Some(ic) = fm.visual_icon() {
                self.active_note_icon = ic.to_string();
                self.set_entry_icon(path, Some(ic.to_string()));
            } else {
                self.active_note_icon = self.entry_icon(path).unwrap_or_default().to_string();
            }
        } else {
            self.active_note_icon = self.entry_icon(path).unwrap_or_default().to_string();
        }

        // Extrai o nome da nota a partir do caminho
        let title = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".md");
        self.active_note_title = title.to_string();
    }

    /// Define ou atualiza o ícone de uma entrada no catálogo da árvore.
    pub fn set_entry_icon(&mut self, path: &str, icon: Option<String>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.icon = icon;
        }
    }

    /// Retorna o ícone associado a uma entrada no catálogo, se houver.
    pub fn entry_icon(&self, path: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.path == path)
            .and_then(|e| e.icon.as_deref())
    }

    /// Adiciona um arquivo ou pasta ao catálogo da árvore.
    pub fn add_entry(&mut self, entry: FileTreeEntry) {
        if !self.entries.iter().any(|e| e.path == entry.path) {
            self.entries.push(entry);
        }
    }

    /// Cria uma pasta ou subpasta na árvore de arquivos e a expande.
    pub fn create_folder(&mut self, parent_folder: Option<&str>, name: &str) -> Result<String, String> {
        let trimmed = name.trim().trim_matches('/');
        if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
            return Err("Nome de pasta inválido.".to_string());
        }

        let parent_clean = parent_folder.map(|p| p.trim_matches('/')).filter(|p| !p.is_empty());
        let full_path = match parent_clean {
            Some(parent) => format!("{parent}/{trimmed}"),
            None => trimmed.to_string(),
        };

        if self.entries.iter().any(|e| e.path == full_path) {
            return Err(format!("Já existe um item chamado '{full_path}'."));
        }

        self.add_entry(FileTreeEntry {
            id: full_path.clone(),
            name: trimmed.to_string(),
            path: full_path.clone(),
            is_folder: true,
            parent_path: parent_clean.map(|p| p.to_string()),
            icon: None,
        });

        if let Some(parent) = parent_clean {
            self.expanded_folders.insert(parent.to_string());
        }
        self.expanded_folders.insert(full_path.clone());

        Ok(full_path)
    }

    /// Move um arquivo ou pasta para dentro de `target_folder` (`None` = raiz).
    /// Retorna o novo caminho relativo em caso de sucesso.
    pub fn move_entry(
        &mut self,
        source_path: &str,
        target_folder: Option<&str>,
    ) -> Result<(String, bool), String> {
        let src = source_path.trim_matches('/');
        if src.is_empty() {
            return Err("Caminho de origem vazio.".to_string());
        }

        let Some(src_entry) = self.entries.iter().find(|e| e.path == src).cloned() else {
            return Err(format!("Item não encontrado: {src}"));
        };

        let target_clean = target_folder
            .map(|p| p.trim_matches('/'))
            .filter(|p| !p.is_empty());

        if let Some(dest) = target_clean {
            if !self.entries.iter().any(|e| e.path == dest && e.is_folder) {
                return Err(format!("Pasta de destino inválida: {dest}"));
            }
            if src_entry.is_folder && (dest == src || dest.starts_with(&format!("{src}/"))) {
                return Err("Não é possível mover uma pasta para dentro dela mesma.".to_string());
            }
        }

        if src_entry.parent_path.as_deref() == target_clean {
            return Err("O item já está nesta pasta.".to_string());
        }

        let file_segment = src.rsplit('/').next().unwrap_or(src);
        let new_path = match target_clean {
            Some(dest) => format!("{dest}/{file_segment}"),
            None => file_segment.to_string(),
        };

        if self.entries.iter().any(|e| e.path == new_path) {
            return Err(format!("Já existe um item em '{new_path}'."));
        }

        let old_prefix = format!("{src}/");
        let new_prefix = format!("{new_path}/");

        for entry in &mut self.entries {
            if entry.path == src {
                entry.path = new_path.clone();
                entry.id = new_path.clone();
                entry.parent_path = target_clean.map(|p| p.to_string());
            } else if src_entry.is_folder && entry.path.starts_with(&old_prefix) {
                let suffix = &entry.path[old_prefix.len()..];
                let updated = format!("{new_prefix}{suffix}");
                entry.path = updated.clone();
                entry.id = updated;
                if let Some(parent) = &entry.parent_path {
                    if parent == src {
                        entry.parent_path = Some(new_path.clone());
                    } else if parent.starts_with(&old_prefix) {
                        let p_suffix = &parent[old_prefix.len()..];
                        entry.parent_path = Some(format!("{new_prefix}{p_suffix}"));
                    }
                }
            }
        }

        if src_entry.is_folder {
            let old_expanded: Vec<String> = self.expanded_folders.iter().cloned().collect();
            for exp in old_expanded {
                if exp == src {
                    self.expanded_folders.remove(&exp);
                    self.expanded_folders.insert(new_path.clone());
                } else if exp.starts_with(&old_prefix) {
                    self.expanded_folders.remove(&exp);
                    let suffix = &exp[old_prefix.len()..];
                    self.expanded_folders.insert(format!("{new_prefix}{suffix}"));
                }
            }
        }

        if let Some(active) = &self.active_note_path {
            if active == src {
                self.active_note_path = Some(new_path.clone());
            } else if src_entry.is_folder && active.starts_with(&old_prefix) {
                let suffix = &active[old_prefix.len()..];
                self.active_note_path = Some(format!("{new_prefix}{suffix}"));
            }
        }

        if let Some(dest) = target_clean {
            self.expanded_folders.insert(dest.to_string());
        }

        Ok((new_path, src_entry.is_folder))
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
                        icon: None,
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

            let kind = gnosis_core::path::DocumentKind::from_path_str(path_str);
            let clean_name = if let Some(stripped) = file_name.strip_suffix(".canvas") {
                stripped.to_string()
            } else if let Some(stripped) = file_name.strip_suffix(".membrana") {
                stripped.to_string()
            } else if let Some(stripped) = file_name.strip_suffix(".md") {
                stripped.to_string()
            } else if let Some(stripped) = file_name.strip_suffix(".markdown") {
                stripped.to_string()
            } else {
                file_name.to_string()
            };

            let default_icon = if kind.is_membrane() {
                Some(kind.default_icon().to_string())
            } else {
                None
            };

            self.add_entry(FileTreeEntry {
                id: path_str.to_string(),
                name: clean_name,
                path: path_str.to_string(),
                is_folder: false,
                parent_path,
                icon: default_icon,
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

                let icon_str = match &child.icon {
                    Some(ic) if !ic.is_empty() => ic.clone(),
                    _ => {
                        if !child.is_folder {
                            let kind = gnosis_core::path::DocumentKind::from_path_str(&child.path);
                            if kind.is_membrane() {
                                "📜".to_string()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    }
                };

                out.push(TreeItemModel {
                    id: child.id.clone(),
                    name: child.name.clone(),
                    path: child.path.clone(),
                    is_folder: child.is_folder,
                    is_expanded,
                    indent_level: level,
                    is_active,
                    icon: icon_str,
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
        assert_eq!(state.char_count(), 30);
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

    #[test]
    fn test_state_note_icon_and_stickers() {
        let mut state = AppState::new("Vault", "/vault");
        state.populate_from_paths(vec!["Notas/Rust.md"]);

        let note_content = "---\nicon: 🦀\nsticker: ferris.svg\n---\n# Rust Lang";
        state.select_note("Notas/Rust.md", note_content);

        assert_eq!(state.active_note_icon, "🦀");
        assert_eq!(state.active_note_title, "Rust");

        // Expande a pasta para que o item fique visível na árvore
        state.toggle_folder("Notas");
        let visible = state.visible_tree_items();
        let rust_item = visible.iter().find(|i| i.path == "Notas/Rust.md" || i.name == "Rust");
        assert!(rust_item.is_some());
        if let Some(item) = rust_item {
            assert_eq!(item.icon, "🦀");
        }
    }

    #[test]
    fn test_state_membrane_and_canvas_tree_items() {
        let mut state = AppState::new("Grimório", "/vault");
        let paths = vec![
            "Pergaminhos/VisaoGeral.canvas",
            "Pergaminhos/MapaMental.membrana",
            "Notas/Introducao.md",
        ];
        state.populate_from_paths(paths);

        // Abre a pasta Pergaminhos
        state.toggle_folder("Pergaminhos");
        let items = state.visible_tree_items();

        let canvas_item = items.iter().find(|i| i.path == "Pergaminhos/VisaoGeral.canvas");
        assert!(canvas_item.is_some());
        if let Some(c) = canvas_item {
            assert_eq!(c.name, "VisaoGeral");
            assert_eq!(c.icon, "📜");
            assert!(!c.is_folder);
        }

        let membrana_item = items.iter().find(|i| i.path == "Pergaminhos/MapaMental.membrana");
        assert!(membrana_item.is_some());
        if let Some(m) = membrana_item {
            assert_eq!(m.name, "MapaMental");
            assert_eq!(m.icon, "📜");
            assert!(!m.is_folder);
        }
    }

    #[test]
    fn test_state_mobile_drawer_and_mode() {
        let mut state = AppState::new("Grimório Mobile", "/storage/emulated/0/Gnosis");
        assert!(!state.is_mobile);
        assert!(!state.mobile_drawer_open);

        state.set_mobile(true);
        assert!(state.is_mobile);

        // Abre e fecha drawer
        let opened = state.toggle_mobile_drawer();
        assert!(opened);
        assert!(state.mobile_drawer_open);

        state.close_mobile_drawer();
        assert!(!state.mobile_drawer_open);

        // Desativar modo mobile deve forçar fechamento da gaveta
        state.toggle_mobile_drawer();
        assert!(state.mobile_drawer_open);
        state.set_mobile(false);
        assert!(!state.is_mobile);
        assert!(!state.mobile_drawer_open);
    }

    #[test]
    fn test_state_ime_bottom_inset() {
        let mut state = AppState::new("Grimório Mobile", "/storage/emulated/0/Gnosis");
        assert_eq!(state.ime_bottom_inset(), 0.0);

        state.set_ime_bottom_inset(285.5);
        assert_eq!(state.ime_bottom_inset(), 285.5);

        // Valor negativo deve ser limitado a 0.0
        state.set_ime_bottom_inset(-50.0);
        assert_eq!(state.ime_bottom_inset(), 0.0);
    }

    #[test]
    fn test_create_folder_and_move_entry() {
        let mut state = AppState::new("Vault", "/tmp/vault");
        state.populate_from_paths(["Nota1.md", "PastaA/Nota2.md"]);

        // Criar pasta na raiz e subpasta dentro dela
        let f1 = state.create_folder(None, "Projetos").ok();
        assert_eq!(f1.as_deref(), Some("Projetos"));
        let f2 = state.create_folder(Some("Projetos"), "Rust").ok();
        assert_eq!(f2.as_deref(), Some("Projetos/Rust"));

        // Mover arquivo da raiz para subpasta
        let moved = state.move_entry("Nota1.md", Some("Projetos/Rust")).ok();
        assert_eq!(moved, Some(("Projetos/Rust/Nota1.md".to_string(), false)));
        assert!(state.entries.iter().any(|e| e.path == "Projetos/Rust/Nota1.md"));
        assert!(!state.entries.iter().any(|e| e.path == "Nota1.md"));

        // Mover pasta para dentro de outra e depois voltar para a raiz
        let moved_folder = state.move_entry("PastaA", Some("Projetos")).ok();
        assert_eq!(moved_folder, Some(("Projetos/PastaA".to_string(), true)));
        assert!(state.entries.iter().any(|e| e.path == "Projetos/PastaA/Nota2.md"));

        // Não pode mover pasta para dentro dela mesma
        assert!(state.move_entry("Projetos", Some("Projetos/PastaA")).is_err());

        // Mover arquivo de volta para a raiz
        let back_to_root = state.move_entry("Projetos/Rust/Nota1.md", None).ok();
        assert_eq!(back_to_root, Some(("Nota1.md".to_string(), false)));
    }
}

