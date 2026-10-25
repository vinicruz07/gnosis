//! Gnosis Markdown: Parser e extrator puro de Wikilinks, tags, embeds e frontmatter YAML.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Código puro, agnóstico de plataforma e sem I/O direto (compilável nativo e para WebAssembly).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod error;
pub mod frontmatter;
pub mod parser;
pub mod tag;
pub mod wikilink;

pub use error::{MarkdownError, Result};
pub use frontmatter::{parse_frontmatter, Frontmatter};
pub use parser::ParsedMetadata;
pub use tag::scan_tags;
pub use wikilink::{scan_wikilinks, WikiLink};
