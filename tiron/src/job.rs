use std::path::PathBuf;

use hcl_edit::structure::Block;

#[derive(Clone)]
pub struct Job {
    pub block: Block,
    pub imported: Option<PathBuf>,
}

impl Job {}
