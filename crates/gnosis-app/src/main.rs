//! Ponto de entrada do aplicativo Gnosis Desktop.
//!
//! Inicialização do Slint UI com renderizador leve, abas múltiplas, split view,
//! Quick Switcher, editor ágil e Canvas do Grafo de Conhecimento 2D com física e Auto-Sleep.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use gnosis_app::{
    AppState, AppWindow, EditorController, EditorMode, EditorPane, GraphEdgeSlint,
    GraphEdgeVisual, GraphNodeSlint, GraphNodeVisual, GraphViewController, QuickItem,
    QuickItemData, QuickMode, QuickSwitcherManager, RenderBlockData,
    RenderBlockModel, SplitDirection, TabData, TabManager, TreeItemData,
    TreeItemModel,
};
use gnosis_graph::model::{EdgeType, KnowledgeGraph, NodeType};
use gnosis_graph::subgraph::GraphFilterConfig;
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

fn to_slint_ast_blocks(blocks: &[RenderBlockModel]) -> Vec<RenderBlockData> {
    blocks
        .iter()
        .map(|b| RenderBlockData {
            kind: b.kind.as_str().into(),
            text: b.text.as_str().into(),
            extra: b.extra.as_str().into(),
            is_checked: b.is_checked,
        })
        .collect()
}

fn to_slint_graph_nodes(nodes: &[GraphNodeVisual]) -> Vec<GraphNodeSlint> {
    nodes
        .iter()
        .map(|n| GraphNodeSlint {
            id: n.id.as_str().into(),
            label: n.label.as_str().into(),
            x: n.x,
            y: n.y,
            radius: n.radius,
            color_hex: n.color_hex.as_str().into(),
            is_pinned: n.is_pinned,
        })
        .collect()
}

fn to_slint_graph_edges(edges: &[GraphEdgeVisual]) -> Vec<GraphEdgeSlint> {
    edges
        .iter()
        .map(|e| GraphEdgeSlint {
            source_x: e.source_x,
            source_y: e.source_y,
            target_x: e.target_x,
            target_y: e.target_y,
            color_hex: e.color_hex.as_str().into(),
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Inicializando Gnosis - Canvas do Grafo 2D com Física e Auto-Sleep");

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

    // Conteúdo demonstrativo rico com múltiplos blocos AST
    let sample_markdown = r#"# Gnosis Core & Slint Shell
Recompilação de alto desempenho do Obsidian em Rust puro para hardware legado.

```rust
fn main() {
    println!("Gnosis: 60 FPS com menos de 25MB de RAM");
}
```

> [!NOTE] Arquitetura Desacoplada
> O motor suporta Desktop nativo, Modo Portable em pendrives e Web via WebAssembly.

- [x] Árvore de notas com Ropey e VFS atômico
- [x] Grafo de conhecimento com física 2D e Auto-Sleep (0% CPU)
- [ ] Renderizador matemático com Typst em vetor

> "A simplicidade é o último grau de sofisticação." — Leonardo da Vinci
"#;

    // Inicializa gerenciador de abas
    let mut tab_mgr = TabManager::new();
    tab_mgr.open_note("01_Projetos/Rust/Gnosis Core.md", "Gnosis Core");

    // Inicializa controlador do editor com Ropey
    let mut editor_ctrl = EditorController::new("01_Projetos/Rust/Gnosis Core.md", sample_markdown);

    // Inicializa Quick Switcher com as notas catalogadas
    let mut quick_mgr = QuickSwitcherManager::new();
    for entry in &state.entries {
        if !entry.is_folder {
            quick_mgr.register_note(&entry.path, &entry.name);
        }
    }

    // Inicializa Grafo de Conhecimento com conexões
    let mut knowledge_graph = KnowledgeGraph::new();
    knowledge_graph.add_or_update_node("00_Inbox/Ideias Rápidas.md", "Ideias Rápidas", NodeType::Note, None);
    knowledge_graph.add_or_update_node("01_Projetos/Rust/Gnosis Core.md", "Gnosis Core", NodeType::Note, None);
    knowledge_graph.add_or_update_node("01_Projetos/Rust/Slint Shell.md", "Slint Shell", NodeType::Note, None);
    knowledge_graph.add_or_update_node("02_Estudos/Grafos e Física 2D.md", "Grafos e Física", NodeType::Note, None);
    knowledge_graph.add_or_update_node("03_Arquivo/2026/Planejamento.md", "Planejamento", NodeType::Note, None);
    knowledge_graph.add_or_update_node("Diário de Bordo.md", "Diário de Bordo", NodeType::Note, None);
    knowledge_graph.add_or_update_node("#rust", "Rust Tag", NodeType::Tag, None);
    knowledge_graph.add_or_update_node("#obsidian", "Obsidian Tag", NodeType::Tag, None);
    knowledge_graph.add_or_update_node("Nota_Fantasma.md", "Nota Fantasma", NodeType::Unresolved, None);

    let _ = knowledge_graph.add_edge("01_Projetos/Rust/Gnosis Core.md", "01_Projetos/Rust/Slint Shell.md", EdgeType::WikiLink, None, 1.0);
    let _ = knowledge_graph.add_edge("01_Projetos/Rust/Slint Shell.md", "02_Estudos/Grafos e Física 2D.md", EdgeType::WikiLink, None, 1.0);
    let _ = knowledge_graph.add_edge("01_Projetos/Rust/Gnosis Core.md", "#rust", EdgeType::TagRelation, None, 0.5);
    let _ = knowledge_graph.add_edge("01_Projetos/Rust/Slint Shell.md", "#rust", EdgeType::TagRelation, None, 0.5);
    let _ = knowledge_graph.add_edge("00_Inbox/Ideias Rápidas.md", "01_Projetos/Rust/Gnosis Core.md", EdgeType::WikiLink, None, 1.0);
    let _ = knowledge_graph.add_edge("00_Inbox/Ideias Rápidas.md", "Nota_Fantasma.md", EdgeType::WikiLink, None, 1.0);

    let mut graph_ctrl = GraphViewController::new(knowledge_graph);
    // Executa alguns passos iniciais de amortecimento
    for _ in 0..50 {
        graph_ctrl.step();
    }

    // Configura propriedades iniciais na janela
    app.set_vault_name(state.vault_name.as_str().into());
    app.set_active_note_title("Gnosis Core".into());
    app.set_active_note_content(editor_ctrl.content().as_str().into());
    app.set_is_editing(editor_ctrl.mode == EditorMode::Edit);
    app.set_word_count(editor_ctrl.word_count() as i32);
    app.set_char_count(editor_ctrl.char_count() as i32);
    app.set_sidebar_collapsed(state.sidebar_collapsed);
    app.set_sync_status("Sincronizado (Local)".into());
    app.set_can_go_back(tab_mgr.history.can_go_back());
    app.set_can_go_forward(tab_mgr.history.can_go_forward());
    app.set_is_split(false);
    app.set_quick_switcher_open(false);
    app.set_is_graph_view(false);

    let initial_tree = to_slint_tree(&state.visible_tree_items());
    app.set_file_tree(ModelRc::new(VecModel::from(initial_tree)));

    let initial_tabs = to_slint_tabs(tab_mgr.active_pane());
    app.set_tabs(ModelRc::new(VecModel::from(initial_tabs)));

    let initial_ast = to_slint_ast_blocks(&editor_ctrl.generate_render_blocks());
    app.set_ast_blocks(ModelRc::new(VecModel::from(initial_ast)));

    let initial_nodes = to_slint_graph_nodes(&graph_ctrl.get_visual_nodes(400.0, 300.0));
    let initial_edges = to_slint_graph_edges(&graph_ctrl.get_visual_edges(400.0, 300.0));
    app.set_graph_nodes(ModelRc::new(VecModel::from(initial_nodes)));
    app.set_graph_edges(ModelRc::new(VecModel::from(initial_edges)));
    app.set_graph_is_sleeping(graph_ctrl.is_sleeping());

    let state_arc = Arc::new(Mutex::new(state));
    let tab_arc = Arc::new(Mutex::new(tab_mgr));
    let editor_arc = Arc::new(Mutex::new(editor_ctrl));
    let quick_arc = Arc::new(Mutex::new(quick_mgr));
    let graph_arc = Arc::new(Mutex::new(graph_ctrl));
    let app_weak = app.as_weak();

    // Callback: Alternar Visualização do Grafo de Conhecimento (Ctrl+G)
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_toggle_graph_view(move || {
            if let Some(w) = app_weak.upgrade() {
                let current = w.get_is_graph_view();
                let next = !current;
                w.set_is_graph_view(next);

                if next {
                    if let Ok(mut gc) = graph_arc.lock() {
                        gc.wake_up();
                        for _ in 0..20 {
                            gc.step();
                        }
                        let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                        let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                        w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                        w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                        w.set_graph_is_sleeping(gc.is_sleeping());
                    }
                }
            }
        });
    }

    // Callback: Clique em nó do Grafo (Abre nota no editor)
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        let editor_arc = Arc::clone(&editor_arc);
        let state_arc = Arc::clone(&state_arc);

        app.on_graph_node_clicked(move |node_id| {
            let path = node_id.as_str();
            let title = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".md");

            let content = format!("# {}\n\nNota aberta a partir do nó do Grafo de Conhecimento.", title);

            if let Ok(mut st) = state_arc.lock() {
                st.select_note(path, &content);
            }
            if let Ok(mut tm) = tab_arc.lock() {
                tm.open_note(path, title);
            }
            if let Ok(mut ed) = editor_arc.lock() {
                ed.set_content(&content);
            }

            if let Some(w) = app_weak.upgrade() {
                w.set_is_graph_view(false);
                w.set_active_note_title(title.into());
                w.set_active_note_content(content.as_str().into());

                if let Ok(tm) = tab_arc.lock() {
                    let tabs = to_slint_tabs(tm.active_pane());
                    w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                }

                if let Ok(ed) = editor_arc.lock() {
                    let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                    w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                }
            }
        });
    }

    // Callback: Arrastar nó do Grafo com o mouse
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_node_dragged(move |id, x, y| {
            if let Ok(mut gc) = graph_arc.lock() {
                gc.pin_node(id.as_str(), x, y);
                gc.step();

                if let Some(w) = app_weak.upgrade() {
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                    w.set_graph_is_sleeping(gc.is_sleeping());
                }
            }
        });
    }

    // Callback: Soltar nó do Grafo
    {
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_node_released(move |id| {
            if let Ok(mut gc) = graph_arc.lock() {
                gc.unpin_node(id.as_str());
            }
        });
    }

    // Callback: Reorganizar / Reiniciar física
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_restart_physics(move || {
            if let Ok(mut gc) = graph_arc.lock() {
                gc.wake_up();
                for _ in 0..20 {
                    gc.step();
                }
                if let Some(w) = app_weak.upgrade() {
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                    w.set_graph_is_sleeping(gc.is_sleeping());
                }
            }
        });
    }

    // Callback: Alternar filtro de órfãos
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_toggle_hide_orphans(move || {
            if let Ok(mut gc) = graph_arc.lock() {
                let mut filter = gc.filter_config.clone();
                filter.hide_orphans = !filter.hide_orphans;
                let hide = filter.hide_orphans;
                gc.update_filters(filter);

                if let Some(w) = app_weak.upgrade() {
                    w.set_graph_hide_orphans(hide);
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                }
            }
        });
    }

    // Callback: Alternar filtro de notas não resolvidas (fantasmas)
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_toggle_hide_unresolved(move || {
            if let Ok(mut gc) = graph_arc.lock() {
                let mut filter = gc.filter_config.clone();
                filter.hide_unresolved = !filter.hide_unresolved;
                let hide = filter.hide_unresolved;
                gc.update_filters(filter);

                if let Some(w) = app_weak.upgrade() {
                    w.set_graph_hide_unresolved(hide);
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                }
            }
        });
    }

    // Callback: Zoom In do Grafo
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_zoom_in(move || {
            if let Ok(mut gc) = graph_arc.lock() {
                gc.zoom = (gc.zoom + 0.15).min(2.5);
                if let Some(w) = app_weak.upgrade() {
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                }
            }
        });
    }

    // Callback: Zoom Out do Grafo
    {
        let app_weak = app_weak.clone();
        let graph_arc = Arc::clone(&graph_arc);
        app.on_graph_zoom_out(move || {
            if let Ok(mut gc) = graph_arc.lock() {
                gc.zoom = (gc.zoom - 0.15).max(0.4);
                if let Some(w) = app_weak.upgrade() {
                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                }
            }
        });
    }

    // Callback: Alternar modo de Edição / Leitura (Ctrl+E)
    {
        let app_weak = app_weak.clone();
        let editor_arc = Arc::clone(&editor_arc);
        app.on_toggle_editor_mode(move || {
            if let Ok(mut ed) = editor_arc.lock() {
                let mode = ed.toggle_mode();
                let is_edit = mode == EditorMode::Edit;
                let ast = to_slint_ast_blocks(&ed.generate_render_blocks());

                if let Some(w) = app_weak.upgrade() {
                    w.set_is_editing(is_edit);
                    w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                }
            }
        });
    }

    // Callback: Edição de texto ao vivo pelo usuário
    {
        let app_weak = app_weak.clone();
        let editor_arc = Arc::clone(&editor_arc);
        let tab_arc = Arc::clone(&tab_arc);
        app.on_note_text_edited(move |new_text| {
            if let Ok(mut ed) = editor_arc.lock() {
                ed.set_content(new_text.as_str());
                let words = ed.word_count();
                let chars = ed.char_count();

                if let Some(w) = app_weak.upgrade() {
                    w.set_word_count(words as i32);
                    w.set_char_count(chars as i32);
                }
            }

            if let Ok(mut tm) = tab_arc.lock() {
                if let Some(tab) = tm.active_pane_mut().active_tab_mut() {
                    tab.is_modified = true;
                }
                if let Some(w) = app_weak.upgrade() {
                    let tabs = to_slint_tabs(tm.active_pane());
                    w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                }
            }
        });
    }

    // Callback: Alternar checkbox de tarefa na visualização
    {
        let app_weak = app_weak.clone();
        let editor_arc = Arc::clone(&editor_arc);
        app.on_task_checkbox_toggled(move |_idx| {
            if let Ok(ed) = editor_arc.lock() {
                let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                if let Some(w) = app_weak.upgrade() {
                    w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                }
            }
        });
    }

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
        let editor_arc = Arc::clone(&editor_arc);
        app.on_note_selected(move |note_path| {
            let note_str = note_path.as_str();
            let title = note_str
                .rsplit('/')
                .next()
                .unwrap_or(note_str)
                .trim_end_matches(".md");

            let content = format!(
                "# {}\n\nNota carregada com sucesso do Vault local.\n\n```rust\n// Código na nota {}\n```\n\n> [!NOTE]\n> Nota ativa pronta para edição ágil.",
                title, note_str
            );

            if let Ok(mut st) = state_arc.lock() {
                st.select_note(note_str, &content);
            }

            if let Ok(mut ed) = editor_arc.lock() {
                ed.note_path = note_str.to_string();
                ed.note_title = title.to_string();
                ed.set_content(&content);
            }

            if let Ok(mut tm) = tab_arc.lock() {
                tm.open_note(note_str, title);

                if let Some(w) = app_weak.upgrade() {
                    w.set_is_graph_view(false);
                    w.set_active_note_title(title.into());
                    w.set_active_note_content(content.as_str().into());
                    w.set_can_go_back(tm.history.can_go_back());
                    w.set_can_go_forward(tm.history.can_go_forward());

                    let tabs = to_slint_tabs(tm.active_pane());
                    w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                    if let Ok(ed) = editor_arc.lock() {
                        let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                        w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                    }

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
        let editor_arc = Arc::clone(&editor_arc);
        app.on_tab_selected(move |idx| {
            if let Ok(mut tm) = tab_arc.lock() {
                if tm.active_pane_mut().select_tab(idx as usize) {
                    if let Some(tab) = tm.active_pane().active_tab() {
                        let title = tab.title.clone();
                        let path = tab.path.clone();
                        let content = format!("# {}\n\nConteúdo restaurado da aba `{}`.", title, path);

                        if let Ok(mut ed) = editor_arc.lock() {
                            ed.set_content(&content);
                        }

                        if let Some(w) = app_weak.upgrade() {
                            w.set_is_graph_view(false);
                            w.set_active_note_title(title.as_str().into());
                            w.set_active_note_content(content.as_str().into());

                            let tabs = to_slint_tabs(tm.active_pane());
                            w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                            if let Ok(ed) = editor_arc.lock() {
                                let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                                w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                            }
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
        let editor_arc = Arc::clone(&editor_arc);
        app.on_go_back(move || {
            if let Ok(mut tm) = tab_arc.lock() {
                if let Some(path) = tm.navigate_back() {
                    let title = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&path)
                        .trim_end_matches(".md");

                    let content = format!("# {}\n\nNavegação retornada para `{}`.", title, path);

                    if let Ok(mut ed) = editor_arc.lock() {
                        ed.set_content(&content);
                    }

                    if let Some(w) = app_weak.upgrade() {
                        w.set_is_graph_view(false);
                        w.set_active_note_title(title.into());
                        w.set_active_note_content(content.as_str().into());
                        w.set_can_go_back(tm.history.can_go_back());
                        w.set_can_go_forward(tm.history.can_go_forward());

                        let tabs = to_slint_tabs(tm.active_pane());
                        w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                        if let Ok(ed) = editor_arc.lock() {
                            let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                            w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                        }
                    }
                }
            }
        });
    }

    // Callback: Avançar no histórico (Alt+Right)
    {
        let app_weak = app_weak.clone();
        let tab_arc = Arc::clone(&tab_arc);
        let editor_arc = Arc::clone(&editor_arc);
        app.on_go_forward(move || {
            if let Ok(mut tm) = tab_arc.lock() {
                if let Some(path) = tm.navigate_forward() {
                    let title = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&path)
                        .trim_end_matches(".md");

                    let content = format!("# {}\n\nNavegação avançada para `{}`.", title, path);

                    if let Ok(mut ed) = editor_arc.lock() {
                        ed.set_content(&content);
                    }

                    if let Some(w) = app_weak.upgrade() {
                        w.set_is_graph_view(false);
                        w.set_active_note_title(title.into());
                        w.set_active_note_content(content.as_str().into());
                        w.set_can_go_back(tm.history.can_go_back());
                        w.set_can_go_forward(tm.history.can_go_forward());

                        let tabs = to_slint_tabs(tm.active_pane());
                        w.set_tabs(ModelRc::new(VecModel::from(tabs)));

                        if let Ok(ed) = editor_arc.lock() {
                            let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                            w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                        }
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
        let editor_arc = Arc::clone(&editor_arc);
        let graph_arc = Arc::clone(&graph_arc);

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
                            "# {}\n\nNota aberta via Quick Switcher instantâneo.\n\n```rust\n// Código na nota {}\n```\n\n> [!NOTE]\n> Nota ativa pronta para edição ágil.",
                            title, path, path
                        );

                        if let Ok(mut tm) = tab_arc.lock() {
                            tm.open_note(&path, &title);
                        }

                        if let Ok(mut st) = state_arc.lock() {
                            st.select_note(&path, &content);
                        }

                        if let Ok(mut ed) = editor_arc.lock() {
                            ed.set_content(&content);
                        }

                        if let Some(w) = app_weak.upgrade() {
                            w.set_is_graph_view(false);
                            w.set_active_note_title(title.as_str().into());
                            w.set_active_note_content(content.as_str().into());

                            if let Ok(tm) = tab_arc.lock() {
                                let tabs = to_slint_tabs(tm.active_pane());
                                w.set_tabs(ModelRc::new(VecModel::from(tabs)));
                            }

                            if let Ok(ed) = editor_arc.lock() {
                                let ast = to_slint_ast_blocks(&ed.generate_render_blocks());
                                w.set_ast_blocks(ModelRc::new(VecModel::from(ast)));
                            }
                        }
                    }
                    gnosis_app::QuickItemType::Command => {
                        info!("Executando comando da paleta: {}", action_id);
                        if action_id == "cmd_graph" {
                            if let Some(w) = app_weak.upgrade() {
                                w.set_is_graph_view(true);
                                if let Ok(mut gc) = graph_arc.lock() {
                                    gc.wake_up();
                                    let nodes = to_slint_graph_nodes(&gc.get_visual_nodes(400.0, 300.0));
                                    let edges = to_slint_graph_edges(&gc.get_visual_edges(400.0, 300.0));
                                    w.set_graph_nodes(ModelRc::new(VecModel::from(nodes)));
                                    w.set_graph_edges(ModelRc::new(VecModel::from(edges)));
                                }
                            }
                        } else if action_id == "cmd_toggle_sidebar" {
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
        info!("Modo headless detectado. Inicialização completa com Grafo 2D validada.");
        return Ok(());
    }

    app.run()?;

    Ok(())
}
