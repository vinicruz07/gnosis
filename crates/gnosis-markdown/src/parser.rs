use crate::frontmatter::Frontmatter;
use crate::wikilink::WikiLink;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedNote {
    pub frontmatter: Option<Frontmatter>,
    pub links: Vec<WikiLink>,
    pub tags: Vec<String>,
    pub raw_content: String,
}
