use std::collections::HashMap;

use super::{copy::CopyAction, package::PackageAction, Action};

pub fn all_actions() -> HashMap<String, Box<dyn Action>> {
    [
        Box::<CopyAction>::default() as Box<dyn Action>,
        Box::<PackageAction>::default() as Box<dyn Action>,
    ]
    .into_iter()
    .map(|a| (a.name(), a))
    .collect()
}
