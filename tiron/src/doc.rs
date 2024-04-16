use std::{io::Write, path::PathBuf};

use anyhow::{anyhow, Result};
use itertools::Itertools;
use tiron_node::action::data::all_actions;

pub fn generate_doc() -> Result<()> {
    let path = PathBuf::from("docs/content/docs/actions/");
    if !path.exists() {
        return Err(anyhow!("can't find actions folder"));
    }
    let actions = all_actions();
    for action in actions.values() {
        let doc = action.doc();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.join(format!("{}.md", action.name())))?;
        file.write_all(b"+++\n")?;
        file.write_all(format!("title = \"{}\"\n", action.name()).as_bytes())?;
        file.write_all(b"template = \"docs/section.html\"\n")?;
        file.write_all(b"+++\n\n")?;
        file.write_all(format!("# {}\n\n", action.name()).as_bytes())?;
        file.write_all(format!("{}\n\n", doc.description).as_bytes())?;
        file.write_all(b"### Parameters\n\n")?;
        file.write_all(b"| Parameter      | Description |\n")?;
        file.write_all(b"| -------------- | ----------- |\n")?;
        for param in &doc.params {
            file.write_all(format!("| **{}** <br>", param.name).as_bytes())?;
            file.write_all(
                format!(
                    " {} <br>",
                    param.type_.iter().map(|t| t.to_string()).join(" or ")
                )
                .as_bytes(),
            )?;
            file.write_all(format!("Required: {} |", param.required).as_bytes())?;
            file.write_all(
                format!(
                    " {} |\n",
                    param.description.replace("\n\n", "<br>").replace('\n', " ")
                )
                .as_bytes(),
            )?;
        }
    }
    Ok(())
}
