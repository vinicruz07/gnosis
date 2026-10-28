//! Runtime seguro de sandbox com isolamento de memória e contenção de falhas (pânico e timeout).
//!
//! Em conformidade com os princípios arquiteturais do Gnosis:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Contenção estrita de falhas: pânicos em plugins são isolados e jamais derrubam a aplicação.
//! - Limites rigorosos de memória (RAM) e tempo de execução (Timeout).
//! - Isolamento de sistema de arquivos via [`SandboxFileSystem`].

use crate::error::{PluginError, Result};
use crate::manifest::PluginPermission;
use async_trait::async_trait;
use gnosis_core::error::{CoreError, Result as CoreResult};
use gnosis_core::path::VaultPath;
use gnosis_core::vfs::{FileMetadata, FileSystemProvider};
use serde::{Deserialize, Serialize};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Configurações de restrição e limites da sandbox para um plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSandboxConfig {
    /// Limite máximo de memória alocada dinamicamente pelo plugin em bytes (padrão: 16 MB).
    pub max_memory_bytes: usize,
    /// Tempo máximo permitido para uma operação síncrona ou assíncrona em milissegundos (padrão: 5.000 ms).
    pub timeout_millis: u64,
    /// Exige isolamento estrito: só permite leitura/escrita fora da pasta do plugin com permissão explícita.
    pub enforce_strict_fs_isolation: bool,
}

impl Default for PluginSandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024, // 16 MB
            timeout_millis: 5000,               // 5 segundos
            enforce_strict_fs_isolation: true,
        }
    }
}

/// Runtime de sandbox associado a uma instância de plugin.
#[derive(Debug)]
pub struct PluginSandbox {
    plugin_id: String,
    config: PluginSandboxConfig,
    allocated_memory: Arc<AtomicUsize>,
}

impl PluginSandbox {
    /// Inicializa a sandbox para um plugin com configurações personalizadas.
    pub fn new(plugin_id: impl Into<String>, config: PluginSandboxConfig) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            config,
            allocated_memory: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Inicializa a sandbox com as configurações padrão seguras.
    pub fn with_default(plugin_id: impl Into<String>) -> Self {
        Self::new(plugin_id, PluginSandboxConfig::default())
    }

    /// Retorna o ID do plugin dono desta sandbox.
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Retorna a quantidade atual de memória em bytes alocada pelo plugin.
    pub fn current_memory_usage(&self) -> usize {
        self.allocated_memory.load(Ordering::Relaxed)
    }

    /// Registra uma alocação de memória garantindo que não ultrapasse o limite configurado.
    pub fn allocate_memory(&self, bytes: usize) -> Result<()> {
        let current = self.allocated_memory.load(Ordering::Relaxed);
        let new_total = current.saturating_add(bytes);

        if new_total > self.config.max_memory_bytes {
            return Err(PluginError::MemoryLimitExceeded(
                new_total,
                self.config.max_memory_bytes,
            ));
        }

        self.allocated_memory.store(new_total, Ordering::Relaxed);
        Ok(())
    }

    /// Libera memória alocada previamente pelo plugin.
    pub fn deallocate_memory(&self, bytes: usize) {
        let current = self.allocated_memory.load(Ordering::Relaxed);
        let new_total = current.saturating_sub(bytes);
        self.allocated_memory.store(new_total, Ordering::Relaxed);
    }

    /// Executa uma função assíncrona com interceptação de pânico e aplicação estrita de timeout.
    ///
    /// Se o código disparar `panic!`, o pânico é contido e convertido em `PluginError::PanicIntercepted`.
    /// Se a execução demorar mais do que `timeout_millis`, é cancelada com `PluginError::ExecutionTimeout`.
    pub async fn execute_safe<F, Fut, R>(&self, action: F) -> Result<R>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<R>> + Send + 'static,
        R: Send + 'static,
    {
        let timeout_dur = Duration::from_millis(self.config.timeout_millis);

        let handle = tokio::task::spawn(async move {
            action().await
        });

        match timeout(timeout_dur, handle).await {
            Ok(join_res) => match join_res {
                Ok(task_result) => task_result,
                Err(join_err) => {
                    if join_err.is_panic() {
                        let panic_msg = if let Ok(panic_payload) = join_err.try_into_panic() {
                            if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "Pânico com payload desconhecido".to_string()
                            }
                        } else {
                            "Pânico na tarefa assíncrona".to_string()
                        };

                        tracing::error!("Pânico interceptado na sandbox do plugin: {}", panic_msg);
                        Err(PluginError::PanicIntercepted(panic_msg))
                    } else {
                        Err(PluginError::LifecycleError(
                            self.plugin_id.clone(),
                            "Tarefa cancelada prematuramente".to_string(),
                        ))
                    }
                }
            },
            Err(_) => Err(PluginError::ExecutionTimeout(self.config.timeout_millis)),
        }
    }

    /// Executa uma rotina síncrona com interceptação de pânicos via `catch_unwind`.
    pub fn execute_sync_safe<F, R>(&self, action: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + std::panic::UnwindSafe,
    {
        match std::panic::catch_unwind(AssertUnwindSafe(action)) {
            Ok(result) => result,
            Err(panic_payload) => {
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Pânico com payload desconhecido".to_string()
                };

                tracing::error!("Pânico síncrono interceptado na sandbox: {}", panic_msg);
                Err(PluginError::PanicIntercepted(panic_msg))
            }
        }
    }

    /// Valida se um caminho do Vault pode ser acessado de acordo com as permissões do plugin.
    pub fn check_path_access(
        &self,
        path: &VaultPath,
        is_write: bool,
        permissions: &[PluginPermission],
    ) -> Result<()> {
        let path_str = path.as_str();

        // 1. Acesso à própria pasta de dados do plugin é sempre permitido: .gnosis/plugins/<plugin_id>/...
        let own_data_prefix = format!(".gnosis/plugins/{}", self.plugin_id);
        if path_str == own_data_prefix || path_str.starts_with(&format!("{}/", own_data_prefix)) {
            return Ok(());
        }

        // 2. Acesso à raiz de plugins de terceiros (.gnosis/plugins/) é estritamente proibido
        if path_str.starts_with(".gnosis/plugins/") {
            return Err(PluginError::SandboxViolation(format!(
                "O plugin '{}' tentou acessar dados privados de outro plugin: '{}'",
                self.plugin_id, path_str
            )));
        }

        // 3. Acesso à pasta de configurações (.gnosis/config.json) é proibido para plugins
        if path_str.starts_with(".gnosis/") {
            return Err(PluginError::SandboxViolation(format!(
                "Acesso a configurações internas do sistema negado: '{}'",
                path_str
            )));
        }

        // 4. Acesso ao conteúdo do Vault requer permissão explícita
        if is_write {
            if !permissions.contains(&PluginPermission::WriteNotes) {
                return Err(PluginError::PermissionDenied(
                    PluginPermission::WriteNotes.to_string(),
                    self.plugin_id.clone(),
                ));
            }
        } else if !permissions.contains(&PluginPermission::ReadNotes) {
            return Err(PluginError::PermissionDenied(
                PluginPermission::ReadNotes.to_string(),
                self.plugin_id.clone(),
            ));
        }

        Ok(())
    }
}

/// Provedor VFS protegido por sandbox que intercepta todas as operações de disco.
pub struct SandboxFileSystem<P: FileSystemProvider> {
    inner: Arc<P>,
    sandbox: Arc<PluginSandbox>,
    permissions: Vec<PluginPermission>,
}

impl<P: FileSystemProvider> SandboxFileSystem<P> {
    /// Constrói um VFS encapsulado com verificação de permissões da sandbox.
    pub fn new(inner: Arc<P>, sandbox: Arc<PluginSandbox>, permissions: Vec<PluginPermission>) -> Self {
        Self {
            inner,
            sandbox,
            permissions,
        }
    }
}

#[async_trait]
impl<P: FileSystemProvider> FileSystemProvider for SandboxFileSystem<P> {
    async fn read_file(&self, path: &VaultPath) -> CoreResult<Vec<u8>> {
        self.sandbox
            .check_path_access(path, false, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.read_file(path).await
    }

    async fn write_file_atomic(&self, path: &VaultPath, data: &[u8]) -> CoreResult<()> {
        self.sandbox
            .check_path_access(path, true, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.write_file_atomic(path, data).await
    }

    async fn exists(&self, path: &VaultPath) -> CoreResult<bool> {
        self.sandbox
            .check_path_access(path, false, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.exists(path).await
    }

    async fn remove_file(&self, path: &VaultPath) -> CoreResult<()> {
        self.sandbox
            .check_path_access(path, true, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.remove_file(path).await
    }

    async fn create_dir_all(&self, path: &VaultPath) -> CoreResult<()> {
        self.sandbox
            .check_path_access(path, true, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.create_dir_all(path).await
    }

    async fn list_dir(&self, path: &VaultPath) -> CoreResult<Vec<VaultPath>> {
        self.sandbox
            .check_path_access(path, false, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.list_dir(path).await
    }

    async fn metadata(&self, path: &VaultPath) -> CoreResult<FileMetadata> {
        self.sandbox
            .check_path_access(path, false, &self.permissions)
            .map_err(|e| CoreError::PermissionDenied(e.to_string()))?;

        self.inner.metadata(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnosis_core::vfs::MemoryFileSystem;

    #[tokio::test]
    async fn test_sandbox_panic_containment() {
        let sandbox = PluginSandbox::with_default("buggy-plugin");

        let result = sandbox
            .execute_safe(|| async {
                panic!("Erro crítico simulado dentro do script do plugin!");
                #[allow(unreachable_code)]
                Ok(42)
            })
            .await;

        assert!(result.is_err());
        if let Err(PluginError::PanicIntercepted(msg)) = result {
            assert!(msg.contains("Erro crítico simulado"));
        } else {
            panic!("Esperava PanicIntercepted, obteve: {:?}", result);
        }
    }

    #[tokio::test]
    async fn test_sandbox_execution_timeout() {
        let config = PluginSandboxConfig {
            max_memory_bytes: 1024,
            timeout_millis: 100, // 100ms
            enforce_strict_fs_isolation: true,
        };
        let sandbox = PluginSandbox::new("slow-plugin", config);

        let result = sandbox
            .execute_safe(|| async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok("concluiu tarde demais")
            })
            .await;

        assert!(result.is_err());
        assert_eq!(result, Err(PluginError::ExecutionTimeout(100)));
    }

    #[tokio::test]
    async fn test_sandbox_memory_limits() {
        let config = PluginSandboxConfig {
            max_memory_bytes: 500, // 500 bytes apenas
            timeout_millis: 1000,
            enforce_strict_fs_isolation: true,
        };
        let sandbox = PluginSandbox::new("greedy-plugin", config);

        assert!(sandbox.allocate_memory(300).is_ok());
        assert_eq!(sandbox.current_memory_usage(), 300);

        // Ultrapassa 500 bytes (300 + 300 = 600 > 500)
        let err = sandbox.allocate_memory(300);
        assert!(err.is_err());
        assert_eq!(err, Err(PluginError::MemoryLimitExceeded(600, 500)));

        sandbox.deallocate_memory(150);
        assert_eq!(sandbox.current_memory_usage(), 150);
    }

    #[tokio::test]
    async fn test_sandbox_filesystem_isolation_rules() {
        let sandbox = Arc::new(PluginSandbox::with_default("isolated-plugin"));
        let inner_vfs = Arc::new(MemoryFileSystem::new());

        // Plugin sem permissões de leitura/escrita de notas
        let restricted_vfs = SandboxFileSystem::new(inner_vfs.clone(), sandbox.clone(), vec![]);

        let private_data = VaultPath::parse(".gnosis/plugins/isolated-plugin/state.json").unwrap();
        let other_plugin_data = VaultPath::parse(".gnosis/plugins/other-plugin/secret.json").unwrap();
        let user_note = VaultPath::parse("Notas/Diario.md").unwrap();

        // 1. Escrever na própria pasta de dados é PERMITIDO mesmo sem permissão global
        assert!(restricted_vfs.write_file_atomic(&private_data, b"{\"theme\":\"dark\"}").await.is_ok());

        // 2. Escrever ou ler pasta de outro plugin é PROIBIDO (SandboxViolation)
        assert!(restricted_vfs.write_file_atomic(&other_plugin_data, b"hack").await.is_err());

        // 3. Ler ou escrever notas do usuário sem permissão é PROIBIDO (PermissionDenied)
        assert!(restricted_vfs.write_file_atomic(&user_note, b"# Minha nota").await.is_err());
        assert!(restricted_vfs.read_file(&user_note).await.is_err());

        // Agora com permissão ReadNotes
        let reader_vfs = SandboxFileSystem::new(
            inner_vfs.clone(),
            sandbox.clone(),
            vec![PluginPermission::ReadNotes],
        );
        // Pode ler, mas ainda não pode escrever
        assert!(reader_vfs.write_file_atomic(&user_note, b"escrever").await.is_err());
    }
}
