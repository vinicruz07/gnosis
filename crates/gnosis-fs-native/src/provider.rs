use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct NativeFileSystem {
    pub root: PathBuf,
}

impl NativeFileSystem {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}
