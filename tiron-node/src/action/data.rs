use std::collections::HashMap;

use super::{
    command::CommandAction, copy::CopyAction, file::FileAction, git::GitAction,
    package::PackageAction, Action,
};

pub fn all_actions() -> HashMap<String, Box<dyn Action>> {
    [
        Box::<CopyAction>::default() as Box<dyn Action>,
        Box::<PackageAction>::default() as Box<dyn Action>,
        Box::<CommandAction>::default() as Box<dyn Action>,
        Box::<FileAction>::default() as Box<dyn Action>,
        Box::<GitAction>::default() as Box<dyn Action>,
    ]
    .into_iter()
    .map(|a| (a.name(), a))
    .collect()
}
