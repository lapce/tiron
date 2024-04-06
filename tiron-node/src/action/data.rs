use std::collections::HashMap;

use super::{copy::CopyAction, package::PackageAction, Action};

pub fn all_actions() -> HashMap<String, Box<dyn Action>> {
    [
        ("copy".to_string(), Box::new(CopyAction) as Box<dyn Action>),
        (
            "package".to_string(),
            Box::new(PackageAction) as Box<dyn Action>,
        ),
    ]
    .into_iter()
    .collect()
}
