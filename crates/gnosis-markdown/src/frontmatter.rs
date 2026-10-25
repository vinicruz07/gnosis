use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}
