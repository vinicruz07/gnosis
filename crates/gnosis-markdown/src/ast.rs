//! Definição da Árvore Sintática Abstrata (AST) em Blocos para o Gnosis.
//!
//! Esta estrutura representa a nota como uma lista plana de blocos tipados,
//! pronta para ser consumida e desenhada instantaneamente pela interface gráfica (Slint)
//! sem a necessidade de reconstruir layout HTML pesado.

use crate::frontmatter::Frontmatter;
use crate::wikilink::WikiLink;
use serde::{Deserialize, Serialize};

/// Tipos de caixas de destaque (Callouts) suportadas no padrão Obsidian / GitHub Alerts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
    Info,
    Success,
    Question,
    Quote,
    Custom(String),
}

impl CalloutKind {
    /// Faz o parse a partir do identificador do callout (ex: `"NOTE"`, `"[!NOTE]"`, `"tip"`).
    pub fn parse(s: &str) -> Self {
        let cleaned = s
            .trim()
            .trim_start_matches("[!")
            .trim_end_matches(']')
            .to_ascii_uppercase();

        match cleaned.as_str() {
            "NOTE" => Self::Note,
            "TIP" => Self::Tip,
            "IMPORTANT" => Self::Important,
            "WARNING" => Self::Warning,
            "CAUTION" | "DANGER" => Self::Caution,
            "INFO" => Self::Info,
            "SUCCESS" | "CHECK" | "DONE" => Self::Success,
            "QUESTION" | "HELP" | "FAQ" => Self::Question,
            "QUOTE" | "CITE" => Self::Quote,
            other => Self::Custom(other.to_string()),
        }
    }

    /// Retorna o título padrão caso o usuário não forneça um título customizado.
    pub fn default_title(&self) -> &str {
        match self {
            Self::Note => "Nota",
            Self::Tip => "Dica",
            Self::Important => "Importante",
            Self::Warning => "Atenção",
            Self::Caution => "Cuidado",
            Self::Info => "Informação",
            Self::Success => "Sucesso",
            Self::Question => "Dúvida",
            Self::Quote => "Citação",
            Self::Custom(c) => c.as_str(),
        }
    }
}

/// Item individual de uma lista de tarefas interativa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    /// Se a caixa está marcada (`- [x]`) ou desmarcada (`- [ ]`).
    pub checked: bool,
    /// Texto descritivo da tarefa.
    pub text: String,
    /// Links contidos dentro do texto da tarefa.
    pub links: Vec<WikiLink>,
    /// Tags contidas dentro do texto da tarefa.
    pub tags: Vec<String>,
}

/// Representação de um bloco estruturado da nota.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkdownBlock {
    /// Cabeçalho (H1 até H6) com âncora opcional.
    Heading {
        level: u8,
        text: String,
        anchor: Option<String>,
    },
    /// Parágrafo de texto corrido com metadados de links e tags inline.
    Paragraph {
        text: String,
        links: Vec<WikiLink>,
        tags: Vec<String>,
    },
    /// Bloco de código delimitado por cercas triplas (` ``` `).
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    /// Caixa de destaque estilizada (`> [!NOTE]`).
    Callout {
        kind: CalloutKind,
        title: String,
        content: Vec<MarkdownBlock>,
    },
    /// Lista de tarefas interativas com checkboxes.
    TaskList {
        items: Vec<TaskItem>,
    },
    /// Bloco de citação simples (`> ...`).
    Blockquote {
        lines: Vec<String>,
    },
    /// Equação matemática em bloco centralizada (`$$ ... $$`).
    MathBlock {
        formula: String,
    },
    /// Tabela formatada em linhas e colunas.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Linha horizontal divisória (`---`).
    Divider,
}

/// Documento Markdown totalmente analisado e estruturado em blocos para a UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownDoc {
    /// Metadados do cabeçalho YAML da nota, se houver.
    pub frontmatter: Option<Frontmatter>,
    /// Sequência ordenada de blocos para renderização.
    pub blocks: Vec<MarkdownBlock>,
    /// Todos os links internos e embeds encontrados no documento.
    pub all_links: Vec<WikiLink>,
    /// Todas as tags consolidadas (frontmatter + corpo da nota).
    pub all_tags: Vec<String>,
}
