//! Gerenciador de Grimórios (Grimoire & Vault System).
//!
//! Implementação com as seguintes diretrizes:
//! - 100% Rust Seguro (`#![deny(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Criação atômica de pastas no disco (no caminho escolhido pelo usuário).
//! - Inicialização da pasta `.gnosis/` e nota de boas-vindas.
//! - Varredura de arquivos e persistência de metadados.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Informações de metadados de um Grimório registrado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrimoireInfo {
    /// Identificador estável gerado a partir do caminho canônico.
    pub id: String,
    /// Nome amigável de exibição do Grimório.
    pub name: String,
    /// Caminho absoluto da pasta do Grimório no disco rígido.
    pub path: PathBuf,
    /// Timestamp Unix do último acesso em segundos.
    pub last_opened_epoch_secs: u64,
    /// Se foi marcado como favorito pelo usuário.
    pub is_favorite: bool,
}

impl GrimoireInfo {
    /// Cria uma nova entrada a partir de um caminho e nome.
    pub fn new(name: impl Into<String>, path: PathBuf) -> Self {
        let name_str = name.into();
        let id = format!("{:x}", md5_or_simple_hash(path.to_string_lossy().as_bytes()));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id,
            name: name_str,
            path,
            last_opened_epoch_secs: now,
            is_favorite: false,
        }
    }

    /// Atualiza o timestamp de último acesso para o momento presente.
    pub fn touch(&mut self) {
        self.last_opened_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

/// Função de hash simples e determinística para gerar IDs estáveis sem dependências externas.
fn md5_or_simple_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Gerenciador de múltiplos Grimórios (Vaults).
#[derive(Debug, Clone)]
pub struct GrimoireManager {
    /// Grimório atualmente carregado e ativo no editor.
    active_grimoire: Option<GrimoireInfo>,
    /// Lista de todos os Grimórios conhecidos na máquina.
    known_grimoires: Vec<GrimoireInfo>,
    /// Indica se o modal seletor/criador de Grimórios está visível na tela.
    is_modal_open: bool,
    /// Diretório onde o arquivo de configuração `app.json` é armazenado.
    config_dir: PathBuf,
}

impl Default for GrimoireManager {
    fn default() -> Self {
        let base_dir = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let config_dir = base_dir.join(".gnosis");
        Self::with_config_dir(config_dir)
    }
}

impl GrimoireManager {
    /// Inicializa o gerenciador com diretório de configuração customizável.
    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        let mut mgr = Self {
            active_grimoire: None,
            known_grimoires: Vec::new(),
            is_modal_open: false,
            config_dir,
        };
        mgr.load_known_grimoires();
        mgr
    }

    /// Retorna o Grimório ativo, se houver.
    pub fn active_grimoire(&self) -> Option<&GrimoireInfo> {
        self.active_grimoire.as_ref()
    }

    /// Retorna uma referência imutável à lista de grimórios conhecidos.
    pub fn known_grimoires(&self) -> &[GrimoireInfo] {
        &self.known_grimoires
    }

    /// Verifica se o modal seletor de Grimórios está aberto.
    pub fn is_modal_open(&self) -> bool {
        self.is_modal_open
    }

    /// Abre o modal de Grimórios.
    pub fn open_modal(&mut self) {
        self.is_modal_open = true;
    }

    /// Fecha o modal de Grimórios.
    pub fn close_modal(&mut self) {
        self.is_modal_open = false;
    }

    /// Alterna a visibilidade do modal de Grimórios.
    pub fn toggle_modal(&mut self) {
        self.is_modal_open = !self.is_modal_open;
    }

    /// Valida o nome de um novo Grimório.
    pub fn validate_grimoire_name(name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("O nome do Grimório não pode ser vazio.".to_string());
        }
        let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
        if trimmed.chars().any(|c| invalid_chars.contains(&c)) {
            return Err("O nome contém caracteres proibidos (< > : \" / \\ | ? *).".to_string());
        }
        Ok(())
    }

    /// Cria um novo Grimório no disco: cria a subpasta com o nome escolhido dentro do `parent_dir`,
    /// inicializa o diretório `.gnosis/` e cria a nota de boas-vindas padrão.
    pub fn create_grimoire(&mut self, parent_dir: &Path, name: &str) -> Result<GrimoireInfo, String> {
        Self::validate_grimoire_name(name)?;
        let trimmed_name = name.trim();

        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Falha ao criar diretório pai {}: {}", parent_dir.display(), e))?;
        }

        let grimoire_dir = parent_dir.join(trimmed_name);

        if !grimoire_dir.exists() {
            fs::create_dir_all(&grimoire_dir)
                .map_err(|e| format!("Falha ao criar pasta do Grimório {}: {}", grimoire_dir.display(), e))?;
        }

        // Cria a subpasta interna de configuração .gnosis/
        let internal_cfg = grimoire_dir.join(".gnosis");
        if !internal_cfg.exists() {
            let _ = fs::create_dir_all(&internal_cfg);
        }

        // Cria nota inicial de boas-vindas caso ainda não exista
        let welcome_path = grimoire_dir.join("Boas-vindas ao Grimório.md");
        if !welcome_path.exists() {
            let welcome_content = format!(
                "---\ntitle: {}\nicon: 📖\n---\n# {}\n\nEste Grimório foi criado com o **Gnosis**.\n\n- ⚡ **Desempenho Extremo:** 100% Safe Rust, sem latência.\n- ✍️ **Live Preview:** Edição WYSIWYG reativa por blocos.\n- 🕸️ **Grafo & Conexões:** Wikilinks e navegação bidirecional.\n\nComece adicionando novas notas ou organizando suas ideias em pastas!\n",
                trimmed_name, trimmed_name
            );
            let _ = fs::write(&welcome_path, welcome_content);
        }

        let mut info = GrimoireInfo::new(trimmed_name, grimoire_dir);
        info.touch();

        self.register_and_activate(info.clone());
        Ok(info)
    }

    /// Abre um diretório existente como Grimório.
    pub fn open_existing_grimoire(&mut self, path: &Path) -> Result<GrimoireInfo, String> {
        if !path.exists() || !path.is_dir() {
            return Err(format!("O caminho especificado não existe ou não é um diretório: {}", path.display()));
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Grimório")
            .to_string();

        let mut info = GrimoireInfo::new(name, path.to_path_buf());
        info.touch();

        self.register_and_activate(info.clone());
        Ok(info)
    }

    /// Varre recursivamente o Grimório ativo e retorna os caminhos relativos de todas as notas `.md` e arquivos `.canvas`.
    pub fn scan_active_grimoire_relative_paths(&self) -> Vec<String> {
        let Some(active) = &self.active_grimoire else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        Self::scan_dir_recursive(&active.path, &active.path, &mut paths);
        paths.sort();
        paths
    }

    /// Cria uma nova nota no Grimório ativo com o título e conteúdo fornecidos.
    pub fn create_note_in_active(&self, relative_name: &str, content: &str) -> Result<PathBuf, String> {
        let Some(active) = &self.active_grimoire else {
            return Err("Nenhum Grimório ativo no momento.".to_string());
        };

        let clean_name = if relative_name.ends_with(".md") {
            relative_name.to_string()
        } else {
            format!("{}.md", relative_name)
        };

        let target_path = active.path.join(&clean_name);
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }

        fs::write(&target_path, content)
            .map_err(|e| format!("Falha ao gravar arquivo {}: {}", target_path.display(), e))?;

        Ok(target_path)
    }

    /// Cria uma nova Membrana no Grimório ativo com o nome fornecido (.membrana ou .canvas).
    pub fn create_membrane_in_active(&self, relative_name: &str, content: &str) -> Result<PathBuf, String> {
        let Some(active) = &self.active_grimoire else {
            return Err("Nenhum Grimório ativo no momento.".to_string());
        };

        let clean_name = if relative_name.ends_with(".membrana") || relative_name.ends_with(".canvas") {
            relative_name.to_string()
        } else {
            format!("{}.membrana", relative_name)
        };

        let target_path = active.path.join(&clean_name);
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }

        fs::write(&target_path, content)
            .map_err(|e| format!("Falha ao gravar arquivo de membrana {}: {}", target_path.display(), e))?;

        Ok(target_path)
    }

    /// Função recursiva auxiliar para varrer arquivos no disco ignorando pastas de sistema.
    fn scan_dir_recursive(root: &Path, current: &Path, results: &mut Vec<String>) {
        let Ok(read_dir) = fs::read_dir(current) else {
            return;
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Ignora pastas de metadados e arquivos ocultos
            if file_name.starts_with('.') || file_name == "node_modules" || file_name == "target" {
                continue;
            }

            if path.is_dir() {
                Self::scan_dir_recursive(root, &path, results);
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext.eq_ignore_ascii_case("md")
                    || ext.eq_ignore_ascii_case("markdown")
                    || ext.eq_ignore_ascii_case("canvas")
                    || ext.eq_ignore_ascii_case("membrana")
                {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        results.push(rel_str);
                    }
                }
            }
        }
    }

    /// Registra um Grimório na lista de conhecidos, atualiza ordenação e salva no disco.
    fn register_and_activate(&mut self, info: GrimoireInfo) {
        if let Some(idx) = self.known_grimoires.iter().position(|g| g.path == info.path) {
            self.known_grimoires[idx] = info.clone();
        } else {
            self.known_grimoires.push(info.clone());
        }

        // Ordena: favoritos primeiro, depois os mais recentemente acessados
        self.known_grimoires.sort_by(|a, b| {
            b.is_favorite
                .cmp(&a.is_favorite)
                .then_with(|| b.last_opened_epoch_secs.cmp(&a.last_opened_epoch_secs))
        });

        self.active_grimoire = Some(info);
        self.save_known_grimoires();
    }

    /// Carrega a lista de Grimórios conhecidos a partir do arquivo `app.json`.
    fn load_known_grimoires(&mut self) {
        let app_json = self.config_dir.join("app.json");
        if !app_json.exists() {
            return;
        }

        let Ok(content) = fs::read_to_string(&app_json) else {
            return;
        };

        #[derive(Deserialize)]
        struct RawConfig {
            #[serde(default)]
            known_vaults: Vec<RawVaultEntry>,
            last_active_vault_id: Option<String>,
        }

        #[derive(Deserialize)]
        struct RawVaultEntry {
            id: String,
            name: String,
            path: PathBuf,
            #[serde(default)]
            last_opened_epoch_secs: u64,
            #[serde(default)]
            is_favorite: bool,
        }

        if let Ok(raw) = serde_json::from_str::<RawConfig>(&content) {
            self.known_grimoires = raw
                .known_vaults
                .into_iter()
                .map(|rv| GrimoireInfo {
                    id: rv.id,
                    name: rv.name,
                    path: rv.path,
                    last_opened_epoch_secs: rv.last_opened_epoch_secs,
                    is_favorite: rv.is_favorite,
                })
                .collect();

            if let Some(last_id) = raw.last_active_vault_id {
                self.active_grimoire = self.known_grimoires.iter().find(|g| g.id == last_id).cloned();
            }
        }
    }

    /// Salva a lista de Grimórios conhecidos no arquivo `app.json`.
    fn save_known_grimoires(&self) {
        if !self.config_dir.exists() {
            let _ = fs::create_dir_all(&self.config_dir);
        }

        let app_json = self.config_dir.join("app.json");

        #[derive(Serialize)]
        struct RawConfig<'a> {
            version: u32,
            known_vaults: &'a [GrimoireInfo],
            last_active_vault_id: Option<&'a str>,
            auto_open_last_vault: bool,
        }

        let raw = RawConfig {
            version: 1,
            known_vaults: &self.known_grimoires,
            last_active_vault_id: self.active_grimoire.as_ref().map(|g| g.id.as_str()),
            auto_open_last_vault: true,
        };

        if let Ok(serialized) = serde_json::to_string_pretty(&raw) {
            let _ = fs::write(&app_json, serialized);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_grimoire_name_validation() {
        assert!(GrimoireManager::validate_grimoire_name("Grimório de Alquimia").is_ok());
        assert!(GrimoireManager::validate_grimoire_name("Notas-2026").is_ok());

        assert!(GrimoireManager::validate_grimoire_name("").is_err());
        assert!(GrimoireManager::validate_grimoire_name("   ").is_err());
        assert!(GrimoireManager::validate_grimoire_name("Grimorio/Invalido").is_err());
        assert!(GrimoireManager::validate_grimoire_name("Grimorio:Teste").is_err());
        assert!(GrimoireManager::validate_grimoire_name("Grimorio*").is_err());
    }

    #[test]
    fn test_create_and_scan_grimoire_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_grimoire_{}", md5_or_simple_hash(b"lifecycle")));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let cfg_dir = temp_dir.join(".config_test");
        let mut mgr = GrimoireManager::with_config_dir(cfg_dir.clone());

        let vaults_parent = temp_dir.join("vaults");
        let created = mgr
            .create_grimoire(&vaults_parent, "Meu Grimório Arcano")
            .unwrap_or_else(|e| panic!("Erro inesperado: {}", e));

        assert_eq!(created.name, "Meu Grimório Arcano");
        assert!(created.path.exists());
        assert!(created.path.join(".gnosis").exists());
        assert!(created.path.join("Boas-vindas ao Grimório.md").exists());

        // Grimório ativo deve estar definido
        assert!(mgr.active_grimoire().is_some());
        assert_eq!(mgr.active_grimoire().map(|g| g.name.as_str()), Some("Meu Grimório Arcano"));

        // Varredura de caminhos relativos deve encontrar a nota de boas-vindas
        let scanned = mgr.scan_active_grimoire_relative_paths();
        assert_eq!(scanned, vec!["Boas-vindas ao Grimório.md"]);

        // Cria uma nova nota no grimório ativo
        let note_path = mgr
            .create_note_in_active("Capitulo 1/Introducao", "# Capítulo 1\nO início.")
            .unwrap_or_else(|e| panic!("Erro inesperado: {}", e));
        assert!(note_path.exists());

        let rescanned = mgr.scan_active_grimoire_relative_paths();
        assert_eq!(rescanned.len(), 2);
        assert!(rescanned.contains(&"Boas-vindas ao Grimório.md".to_string()));
        assert!(rescanned.contains(&"Capitulo 1/Introducao.md".to_string()));

        // Limpeza
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_modal_lifecycle() {
        let mut mgr = GrimoireManager::with_config_dir(std::env::temp_dir());
        assert!(!mgr.is_modal_open());

        mgr.open_modal();
        assert!(mgr.is_modal_open());

        mgr.close_modal();
        assert!(!mgr.is_modal_open());

        mgr.toggle_modal();
        assert!(mgr.is_modal_open());
    }

    #[test]
    fn test_switch_multiple_grimoires_and_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_grimoire_{}", md5_or_simple_hash(b"multi_switch")));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let cfg_dir = temp_dir.join(".config_test");
        let vaults_parent = temp_dir.join("vaults");

        // Instância 1: cria dois grimórios
        {
            let mut mgr = GrimoireManager::with_config_dir(cfg_dir.clone());
            let g1 = mgr.create_grimoire(&vaults_parent, "Grimório Alfa").unwrap_or_else(|e| panic!("{}", e));
            let _g2 = mgr.create_grimoire(&vaults_parent, "Grimório Beta").unwrap_or_else(|e| panic!("{}", e));

            assert_eq!(mgr.known_grimoires().len(), 2);
            assert_eq!(mgr.active_grimoire().map(|g| g.name.as_str()), Some("Grimório Beta"));

            // Alterna de volta para Alfa
            let reopened = mgr.open_existing_grimoire(&g1.path).unwrap_or_else(|e| panic!("{}", e));
            assert_eq!(reopened.name, "Grimório Alfa");
            assert_eq!(mgr.active_grimoire().map(|g| g.name.as_str()), Some("Grimório Alfa"));
        }

        // Instância 2: recarrega do disco (app.json) e valida persistência
        {
            let mgr2 = GrimoireManager::with_config_dir(cfg_dir);
            assert_eq!(mgr2.known_grimoires().len(), 2);
            assert_eq!(mgr2.active_grimoire().map(|g| g.name.as_str()), Some("Grimório Alfa"));
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_grimoire_membrane_scanning_and_creation() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_grimoire_{}", md5_or_simple_hash(b"membrane_scan")));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let mut mgr = GrimoireManager::with_config_dir(temp_dir.clone());
        let _ = mgr.create_grimoire(&temp_dir, "Grimório Místico").unwrap_or_else(|e| panic!("{}", e));

        // Cria um arquivo .canvas e um arquivo .membrana
        let membrana_path = mgr
            .create_membrane_in_active("Ideias", "{\"nodes\":[],\"edges\":[]}")
            .unwrap_or_else(|e| panic!("{}", e));
        assert!(membrana_path.ends_with("Ideias.membrana"));

        let canvas_path = mgr
            .create_membrane_in_active("Interoperavel.canvas", "{\"nodes\":[],\"edges\":[]}")
            .unwrap_or_else(|e| panic!("{}", e));
        assert!(canvas_path.ends_with("Interoperavel.canvas"));

        let scanned = mgr.scan_active_grimoire_relative_paths();
        assert!(scanned.contains(&"Ideias.membrana".to_string()));
        assert!(scanned.contains(&"Interoperavel.canvas".to_string()));
        assert!(scanned.contains(&"Boas-vindas ao Grimório.md".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

