# Gnosis 🧠

> **Gnosis** é uma re-implementação (recomp) moderna, ultrarrápida e local-first do **Obsidian** desenvolvida em **Rust**.

---

## 🎯 Motivação & Visão

O Obsidian revolucionou o gerenciamento de conhecimento pessoal (PKM) com seu modelo de arquivos locais em Markdown e links bidirecionais. No entanto, por ser baseado em Electron, sofre com:
- Alto consumo de memória RAM (frequentemente >500MB–1GB);
- Tempo de inicialização perceptível em vaults com milhares de notas;
- Gargalos em operações intensivas de busca textual e renderização de grafos grandes.

O **Gnosis** recria essa experiência com foco em:
- ⚡ **Performance Nativa**: Inicialização instantânea (<100ms) e consumo mínimo de memória.
- 📁 **100% Local-First & Compatível**: Abre e manipula vaults do Obsidian existentes sem necessidade de migração.
- 🔗 **Sintaxe Completa**: Suporte nativo a `[[wikilinks]]`, aliases, embeds `![[anexos]]`, frontmatter YAML e tags.
- 🕸️ **Grafo Bidirecional Rápido**: Indexação e análise de grafo paralela multithreaded.
- 🧩 **Extensibilidade**: Sistema de plugins moderno e seguro.

---

## 🏗️ Estrutura do Workspace (Rust)

O projeto é organizado como um workspace modular de crates Rust em `crates/`:

```
gnosis/
├── Cargo.toml                  # Workspace root manifest
├── .gitignore                  # Regras de ignore (Rust, IDEs e rascunhos)
├── README.md                   # Documentação do projeto
├── discussion.md               # [Ignorado no git] Discussões e decisões de arquitetura
├── crates/
│   ├── gnosis-core/            # Gerenciador de Vault, scanner e file-watcher
│   ├── gnosis-markdown/        # Parser Markdown, AST, Wikilinks e Frontmatter
│   ├── gnosis-graph/           # Grafo de conhecimento, backlinks e relações
│   ├── gnosis-plugins/         # Arquitetura de plugins e extensibilidade
│   └── gnosis-app/             # Ponto de entrada e interface desktop
└── docs/                       # Especificações e documentos de design
```

### Detalhes das Crates

| Crate | Responsabilidade |
|---|---|
| `gnosis-core` | Gerenciamento do ciclo de vida de Vaults locais, scanner de diretórios, detecção de mudanças em tempo real via `notify` e persistência de cache. |
| `gnosis-markdown` | Parser de Markdown, extração de metadados, suporte a sintaxe do Obsidian (`[[wikilinks]]`, tags `#tag`, callouts `[!NOTE]`, blocos e embeds). |
| `gnosis-graph` | Modelagem matemática de nós e arestas bidirecionais via `petgraph`, cálculo de backlinks, nós órfãos e layout de grafo. |
| `gnosis-plugins` | Sistema de plugins desacoplado, manifesto de extensões e barramento de eventos. |
| `gnosis-app` | Binário principal unindo as crates, orquestração e interface de usuário. |

---

## 🚀 Como Executar

### Pré-requisitos
- [Rust toolchain](https://rustup.rs/) (versão estável 1.75+ recomendada)

### Compilando e Executando
Para rodar a aplicação em modo de desenvolvimento:

```bash
cargo run -p gnosis-app
```

Para compilar em modo release com otimizações máximas:

```bash
cargo build --release -p gnosis-app
```

---

## 🗺️ Roadmap de Desenvolvimento

- [x] Estrutura inicial do workspace Rust modular.
- [x] Configuração do `.gitignore` com exclusão de notas de discussão locais.
- [ ] Implementação do leitor e scanner de Vaults em `gnosis-core`.
- [ ] Parser completo de Wikilinks e extração de Frontmatter em `gnosis-markdown`.
- [ ] Algoritmo de indexação de Backlinks e Grafo em `gnosis-graph`.
- [ ] Definição e prototipagem da camada de interface gráfica.
- [ ] Integração de busca full-text ultra-rápida.
- [ ] Suporte a plugins e temas customizados.

---

## 📄 Licença
Distribuído sob licença MIT ou Apache-2.0.
