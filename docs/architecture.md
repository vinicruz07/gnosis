# Arquitetura do Gnosis

## Pipeline de Dados

```text
[ Vault no Disco (.md) ]
         │
         ▼ (File Watcher / notify)
[ gnosis-core: Scanner & Indexer ]
         │
         ▼ (Raw Markdown Content)
[ gnosis-markdown: Parser ] ───► [ Extração de Wikilinks & Frontmatter ]
         │                                      │
         ▼                                      ▼
[ AST / Render View ]                [ gnosis-graph: Petgraph ]
         │                                      │
         └───────────────┬──────────────────────┘
                         ▼
                [ gnosis-app: UI ]
```

## Diretrizes de Design
1. **Zero perda de dados**: O Gnosis opera diretamente sobre o sistema de arquivos. Modificações são atômicas e preservam a integridade dos arquivos `.md`.
2. **Desacoplamento**: Cada crate possui responsabilidade única e pode ser testada isoladamente em testes unitários e de benchmark.
3. **Assíncrono onde importa**: I/O de disco, indexação de múltiplos arquivos e parsing pesado são executados via `tokio` em background threads para não travar a interface.
