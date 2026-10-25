//! Ponto de entrada do aplicativo Gnosis Desktop.
//!
//! Inicialização do Slint UI com renderizador leve, abas múltiplas, split views e Quick Switcher (Ctrl+O).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use gnosis_app::{
    AppState, AppWindow, EditorPane, QuickItem, QuickItemData, QuickMode,
    QuickSwitcherManager, SplitDirection, TabData, TabManager, TreeItemData,
    TreeItemModel,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::{Arc, Mutex};
use tracing::info;

fn to_slint_tree(items: &[TreeItemModel]) -> Vec<TreeItemData> {
    items
        .iter()
        .map(|item| TreeItemData {
            id: item.id.as_str().into(),
            name: item.name.as_str().into(),
            path: item.path.as_str().into(),
            is_folder: item.is_folder,
            is_expanded: item.is_expanded,
            indent_level: item.indent_level,
            is_active: item.is_active,
        })
        .collect()
}

fn to_slint_tabs(pane: &EditorPane) -> Vec<TabData> {
    pane.tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| TabData {
            id: tab.id.as_str().into(),
            title: tab.title.as_str().into(),
            path: tab.path.as_str().into(),
            is_active: idx == pane.active_tab_index,
            is_pinned: tab.is_pinned,
            is_modified: tab.is_modified,
        })
        .collect()
}

fn to_slint_quick_items(items: &[QuickItem], selected_idx: usize) -> Vec<QuickItemData> {
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| QuickItemData {
            id: item.id.as_str().into(),
            title: item.title.as_str().into(),
            subtitle: item.subtitle.as_str().into(),
            icon: item.item_type.icon().into(),
            is_selected: idx == selected_idx,
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Inicializando Gnosis - Quick Switcher (Ctrl+O) & Paleta de Comandos");

    // Instancia janela principal Slint
    let app = AppWindow::new()?;

    // Inicializa estado do Vault e árvore de notas
    let mut state = AppState::new("Meu Vault", "vault");
    let initial_paths = [
        "00_Inbox/Ideias Rápidas.md",
        "01_Projetos/Rust/Gnosis Core.md",
        "01_Projetos/Rust/Slint Shell.md",
        "02_Estudos/Grafos e Física 2D.md",
        "03_Arquivo/2026/Planejamento.md",
        "Diário de Bordo.md",
    ];
    state.populate_from_paths(initial_paths);

    // Inicializa gerenciador de abas
    let mut tab_mgr = TabManager::new();
    tab_mgr.open_note("00_Inbox/Ideias Rápidas.md", "Ideias Rápidas");
    tab_mgr.open_note("01_Projetos/Rust/Gnosis Core.md", "Gnosis Core");

    // Inicializa Quick Switcher com as notas catalogadas
    let mut quick_mgr = QuickSwitcherManager::new();
    for entry in &state.entries {
        if !entry.is_folder {
            quick_mgr.register_note(&entry.path, &entry.name);
        }
    }

    // Configura propriedades iniciais na janela
    app.set_vault_name(state.vault_name.as_str().into());
    app.set_active_note_title("Gnosis Core".into());
    app.set_active_note_content(
        "# Gnosis Core\n\nArquitetura de alto desempenho com buffer Ropey e VFS atômico.".into(),
    );
    app.set_word_count(14);
    app.set_char_count(92);
    app.set_sidebar_collapsed(state.sidebar_collapsed);
    app.set_sync_status("Sincronizado (Local)".into());
    app.set_can_go_back(tab_mgr.history.can_go_back());
    app.set_can_go_forward(tab_mgr.history.can_go_forward());
    app.set_is_split(false);
    app.set_quick_switcher_open(false);

    let initial_tree = to_slint_tree(&state.visible_tree_items());
    app.set_file_tree(ModelRc::new(VecModel::from(initial_tree)));

    let initial_tabs = to_slint_tabs(tab_mgr.active_pane());
    app.set_tabs(ModelRc::new(VecModel::from(initial_tabs)));

    let state_arc = Arc::new(Mutex::new(state));
    let tab_arc = Arc::new(Mutex::new(tab_mgr));
    let quick_arc = Arc::new(Mutex::new(quick_mgr));
    let app_weak = app.as_weak();

    // Callback: Alternar barra lateral
    {
        let app_weak = app_weak.clone();
        let state_arc = Arc::clone(&state_arc);
        app.on_toggle_sidebar(move || {
            if let Ok(mut st) = state_arc.lock() {
                let collapsed = st.toggle_sidebar();
                if let Some(w) = app_weak.upgrade() {
                    w.set_sidebar_collapsed(collapsed);
                }
            }
        });
    }

    // Callback: Alternar pasta na árvore
    {
        let app_weak = app_weak.clone();
        let state_arc = Arc::clone(&state_arc);
        app.on_folder_toggled(move |folder_path| {
            if let Ok(mut st) = state_arc.lock() {
                st.toggle_folder(folder_path.as_str());
                let items = to_slint_tree(&st.visible_tree_items());
                if let Some(w) = app_weak.upgrade() {
                    w.set_file_tree(ModelRc::new(VecModel::from(items)));
                }
            }
        });
    }

    // Callback: Selecionar nota na barra lateral
    {
        let app_weak = app_weak.clone();
        let state_arc = Arc::clone(&state_arc);
        let tab_arc = Arc::clone(&tab_arc);
        app.on_note_selected(move |note_path| {
            let note_str = note_path.as_str();
            let title = note_str
                .rsplit('/')
                .next()
                .unwrap_or(note_str)
                .trim_end_matches(".md");

            let content = format!(
                "# {}\n\nNota carregada com sucesso do Vault local.\n\nCaminho seguro: `{}`\nPronta para edição ágil.",
                title, note_str
            );

            if let Ok(mut st) = state_arc.lock() {
                st.select_note(note_str, &content);
            }

            if let Ok(mut tm) = tab_arc.lock() {
                tm.open_note(note_str, title);

                if let Some(w) = app_weak.upgrade() {
                    w.set_active_note_title(title.into());
                    w.set_active_note_content(content.as_str().into());
                    w.set_can_go_back(tm.history.can_go_back());
                    w.set_can_go_forward(tm.history.can_go_forward());

                    let tabs = to_slint_tabs(tm.active_pane());
                    w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                    if let Ok(st) = state_arc.lock() {
                        let items = to_slint_tree(&st.visible_tree_items());
                        w.set_file_tree(ModelRc::new(VecModel::from(items)));
                    }
                }
            }
        });
    }

    // Callback: Selecionar aba no topo
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        app.on_tab_selected(move |idx| {
            if let Ok(mut tm) = tab_arc.lock() {
                if tm.active_pane_mut().select_tab(idx as usize) {
                    if let Some(tab) = tm.active_pane().active_tab() {
                        let title = tab.title.clone();
                        let path = tab.path.clone();
                        let content = format!("# {}\n\nConteúdo restaurado da aba `{}`.", title, path);

                        if let Some(w) = app_weak.upgrade() {
                            w.set_active_note_title(title.as_str().into());
                            w.set_active_note_content(content.as_str().into());

                            let tabs = to_slint_tabs(tm.active_pane());
                            w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                        }
                    }
                }
            }
        });
    }

    // Callback: Fechar aba
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        app.on_tab_closed(move |idx| {
            if let Ok(mut tm) = tab_arc.lock() {
                let _ = tm.active_pane_mut().close_tab(idx as usize);

                if let Some(w) = app_weak.upgrade() {
                    let tabs = to_slint_tabs(tm.active_pane());
                    w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                    if let Some(active_tab) = tm.active_pane().active_tab() {
                        let title = active_tab.title.clone();
                        let content = format!("# {}\n\nConteúdo da nota ativa.", title);
                        w.set_active_note_title(title.as_str().into());
                        w.set_active_note_content(content.as_str().into());
                    } else {
                        w.set_active_note_title("Sem abas abertas".into());
                        w.set_active_note_content("Abra uma nota na barra lateral ou pressione Ctrl+O para buscar.".into());
                    }
                }
            }
        });
    }

    // Callback: Voltar no histórico (Alt+Left)
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        app.on_go_back(move || {
            if let Ok(mut tm) = tab_arc.lock() {
                if let Some(path) = tm.navigate_back() {
                    let title = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&path)
                        .trim_end_matches(".md");

                    if let Some(w) = app_weak.upgrade() {
                        w.set_active_note_title(title.into());
                        w.set_active_note_content(format!("# {}\n\nNavegação retornada para `{}`.", title, path).into());
                        w.set_can_go_back(tm.history.can_go_back());
                        w.set_can_go_forward(tm.history.can_go_forward());

                        let tabs = to_slint_tabs(tm.active_pane());
                        w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                    }
                }
            }
        });
    }

    // Callback: Avançar no histórico (Alt+Right)
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        app.on_go_forward(move || {
            if let Ok(mut tm) = tab_arc.lock() {
                if let Some(path) = tm.navigate_forward() {
                    let title = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&path)
                        .trim_end_matches(".md");

                    if let Some(w) = app_weak.upgrade() {
                        w.set_active_note_title(title.into());
                        w.set_active_note_content(format!("# {}\n\nNavegação avançada para `{}`.", title, path).into());
                        w.set_can_go_back(tm.history.can_go_back());
                        w.set_can_go_forward(tm.history.can_go_forward());

                        let tabs = to_slint_tabs(tm.active_pane());
                        w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                    }
                }
            }
        });
    }

    // Callback: Alternar Split View
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        app.on_toggle_split(move || {
            if let Ok(mut tm) = tab_arc.lock() {
                let new_split = if tm.split_direction == SplitDirection::Single {
                    SplitDirection::Horizontal
                } else {
                    SplitDirection::Single
                };

                tm.set_split(new_split);
                let is_split = new_split != SplitDirection::Single;

                if let Some(w) = app_weak.upgrade() {
                    w.set_is_split(is_split);
                    if is_split {
                        w.set_secondary_note_title("Nota em Split View".into());
                        w.set_secondary_note_content(
                            "Visualização dividida lado a lado ativada para produtividade máxima.".into(),
                        );
                    }
                }
            }
        });
    }

    // Callback: Abrir Quick Switcher (Ctrl+O) ou Paleta de Comandos (Ctrl+P)
    {
        let app_weak = app_weak.clone();
        let quick_arc = Arc::clone(&quick_arc);
        app.on_open_quick_switcher(move |mode_str| {
            let mode = if mode_str.as_str() == "commands" {
                QuickMode::Commands
            } else {
                QuickMode::Files
            };

            let placeholder = match mode {
                QuickMode::Files => "Digite para buscar uma nota no Vault (Ctrl+O)...",
                QuickMode::Commands => "Digite um comando para executar (Ctrl+P)...",
            };

            if let Ok(mut qm) = quick_arc.lock() {
                qm.open(mode);
                let slint_items = to_slint_quick_items(&qm.filtered_items, qm.selected_index);

                if let Some(w) = app_weak.upgrade() {
                    w.set_quick_switcher_placeholder(placeholder.into());
                    w.set_quick_items(ModelRc::new(VecModel::from(slint_items)));
                    w.set_quick_selected_index(qm.selected_index as i32);
                    w.set_quick_switcher_open(true);
                }
            }
        });
    }

    // Callback: Atualização do texto de busca no Quick Switcher
    {
        let app_weak = app_weak.clone();
        let quick_arc = Arc::clone(&quick_arc);
        app.on_quick_query_changed(move |query| {
            if let Ok(mut qm) = quick_arc.lock() {
                qm.set_query(query.as_str());
                let slint_items = to_slint_quick_items(&qm.filtered_items, qm.selected_index);

                if let Some(w) = app_weak.upgrade() {
                    w.set_quick_items(ModelRc::new(VecModel::from(slint_items)));
                    w.set_quick_selected_index(qm.selected_index as i32);
                }
            }
        });
    }

    // Callback: Seleção de item no Quick Switcher
    {
        let app_weak = app_weak.clone();
        let quick_arc = Arc::clone(&quick_arc);
        let tab_arc = Arc::clone(&tab_arc);
        let state_arc = Arc::clone(&state_arc);

        app.on_quick_item_clicked(move |idx| {
            let mut action_opt = None;

            if let Ok(qm) = quick_arc.lock() {
                if let Some(item) = qm.filtered_items.get(idx as usize) {
                    action_opt = Some((item.item_type, item.action_id.clone(), item.title.clone()));
                }
            }

            if let Some((item_type, action_id, title)) = action_opt {
                match item_type {
                    gnosis_app::QuickItemType::Note => {
                        let path = action_id;
                        let content = format!(
                            "# {}\n\nNota aberta via Quick Switcher instantâneo.\n\nCaminho: `{}`",
                            title, path
                        );

                        if let Ok(mut tm) = tab_arc.lock() {
                            tm.open_note(&path, &title);
                        }

                        if let Ok(mut st) = state_arc.lock() {
                            st.select_note(&path, &content);
                        }

                        if let Some(w) = app_weak.upgrade() {
                            w.set_active_note_title(title.as_str().into());
                            w.set_active_note_content(content.as_str().into());

                            if let Ok(tm) = tab_arc.lock() {
                                let tabs = to_slint_tabs(tm.active_pane());
                                w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                            }
                        }
                    }
                    gnosis_app::QuickItemType::Command => {
                        info!("Executando comando da paleta: {}", action_id);
                        // Comandos rápidos executados via paleta
                        if action_id == "cmd_toggle_sidebar" {
                            if let Ok(mut st) = state_arc.lock() {
                                let col = st.toggle_sidebar();
                                if let Some(w) = app_weak.upgrade() {
                                    w.set_sidebar_collapsed(col);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Fecha modal
            if let Ok(mut qm) = quick_arc.lock() {
                qm.close();
            }
            if let Some(w) = app_weak.upgrade() {
                w.set_quick_switcher_open(false);
            }
        });
    }

    // Callback: Fechar Quick Switcher (Esc ou clique fora)
    {
        let app_weak = app_weak.clone();
        let quick_arc = Arc::clone(&quick_arc);
        app.on_quick_close_requested(move || {
            if let Ok(mut qm) = quick_arc.lock() {
                qm.close();
            }
            if let Some(w) = app_weak.upgrade() {
                w.set_quick_switcher_open(false);
            }
        });
    }

    // Suporte a modo headless para testes e CI
    if std::env::var("GNOSIS_HEADLESS").is_ok() {
        info!("Modo headless detectado. Inicialização do Quick Switcher validada.");
        return Ok(());
    }

    app.run()?;

    Ok(())
}
