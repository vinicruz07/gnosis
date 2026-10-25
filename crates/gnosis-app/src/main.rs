//! Ponto de entrada do aplicativo Gnosis Desktop.
//!
//! Inicialização do Slint UI com renderizador leve e conexão com o backend local do Vault.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use gnosis_app::{AppState, AppWindow, TreeItemData, TreeItemModel};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Inicializando Gnosis - Shell de Interface Slint");

    // Instancia janela principal
    let app = AppWindow::new()?;

    // Inicializa estado padrão com árvore de demonstração do Vault
    let mut state = AppState::new("Meu Vault", "vault");
    state.populate_from_paths([
        "00_Inbox/Ideias Rápidas.md",
        "01_Projetos/Rust/Gnosis Core.md",
        "01_Projetos/Rust/Slint Shell.md",
        "02_Estudos/Grafos e Física 2D.md",
        "03_Arquivo/2026/Planejamento.md",
        "Diário de Bordo.md",
    ]);

    // Define propriedades iniciais na UI Slint
    app.set_vault_name(state.vault_name.as_str().into());
    app.set_active_note_title(state.active_note_title.as_str().into());
    app.set_active_note_content(state.active_note_content.as_str().into());
    app.set_word_count(state.word_count() as i32);
    app.set_char_count(state.char_count() as i32);
    app.set_sidebar_collapsed(state.sidebar_collapsed);
    app.set_sync_status("Sincronizado (Local)".into());

    let initial_tree = to_slint_tree(&state.visible_tree_items());
    app.set_file_tree(ModelRc::new(VecModel::from(initial_tree)));

    let state_arc = Arc::new(Mutex::new(state));
    let app_weak = app.as_weak();

    // Callback: Recolher / Expandir barra lateral
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

    // Callback: Abrir / Fechar pasta na árvore
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

    // Callback: Selecionar nota para visualização
    {
        let app_weak = app_weak.clone();
        let state_arc = Arc::clone(&state_arc);
        app.on_note_selected(move |note_path| {
            if let Ok(mut st) = state_arc.lock() {
                let content = format!(
                    "# {}\n\nNota aberta no Gnosis com alto desempenho.\n\nCaminho seguro: `{}`\nPronta para edição instantânea.",
                    note_path.as_str().trim_end_matches(".md"),
                    note_path.as_str()
                );

                st.select_note(note_path.as_str(), &content);

                if let Some(w) = app_weak.upgrade() {
                    w.set_active_note_title(st.active_note_title.as_str().into());
                    w.set_active_note_content(st.active_note_content.as_str().into());
                    w.set_word_count(st.word_count() as i32);
                    w.set_char_count(st.char_count() as i32);

                    let items = to_slint_tree(&st.visible_tree_items());
                    w.set_file_tree(ModelRc::new(VecModel::from(items)));
                }
            }
        });
    }

    // Callback: Criação de nova nota
    {
        let app_weak = app_weak.clone();
        let state_arc = Arc::clone(&state_arc);
        app.on_new_note_requested(move || {
            if let Ok(mut st) = state_arc.lock() {
                let new_path = format!("Sem Título {}.md", st.entries.len());
                st.populate_from_paths([new_path.clone()]);
                st.select_note(&new_path, "# Nova Nota\n\nComece a digitar seu conhecimento...");

                if let Some(w) = app_weak.upgrade() {
                    w.set_active_note_title(st.active_note_title.as_str().into());
                    w.set_active_note_content(st.active_note_content.as_str().into());
                    w.set_word_count(st.word_count() as i32);
                    w.set_char_count(st.char_count() as i32);

                    let items = to_slint_tree(&st.visible_tree_items());
                    w.set_file_tree(ModelRc::new(VecModel::from(items)));
                }
            }
        });
    }

    // Opcional: Se headless para CI/testes, encerra sem abrir loop gráfico
    if std::env::var("GNOSIS_HEADLESS").is_ok() {
        info!("Modo headless detectado. Inicialização do Slint validada com sucesso.");
        return Ok(());
    }

    // Executa loop gráfico principal da interface Slint
    app.run()?;

    Ok(())
}
