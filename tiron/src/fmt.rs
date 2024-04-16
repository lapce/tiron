use std::{fs, path::PathBuf};

use hcl::format::{Format, Formatter};
use tiron_common::error::Error;

pub fn fmt(targets: Vec<String>) -> Result<(), Error> {
    let targets = if targets.is_empty() {
        vec![std::env::current_dir().map_err(|e| Error::new(e.to_string()))?]
    } else {
        targets.iter().map(PathBuf::from).collect()
    };

    for target in targets {
        fmt_target(target)?;
    }

    Ok(())
}

fn fmt_target(path: PathBuf) -> Result<(), Error> {
    if !path.exists() {
        return Error::new(format!("path {} doesn't exist", path.to_string_lossy())).err();
    }

    if path.is_dir() {
        let mut runbooks = Vec::new();
        for path in fs::read_dir(path).map_err(|e| Error::new(e.to_string()))? {
            let path = path.map_err(|e| Error::new(e.to_string()))?;
            if path.file_name().to_string_lossy().ends_with(".tr") {
                runbooks.push(path.path());
            }
        }
        for path in runbooks {
            fmt_runbook(path)?;
        }
    } else {
        fmt_runbook(path)?;
    }

    Ok(())
}

fn fmt_runbook(path: PathBuf) -> Result<(), Error> {
    let data = std::fs::read_to_string(&path).map_err(|e| {
        Error::new(format!(
            "can't read runbook {} error: {e}",
            path.to_string_lossy()
        ))
    })?;
    let body = hcl::parse(&data).map_err(|e| {
        if let hcl::Error::Parse(e) = e {
            Error::from_hcl(e, path.clone())
        } else {
            Error::new(e.to_string())
        }
    })?;
    let mut file = std::fs::File::options()
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|e| Error::new(e.to_string()))?;
    let mut formatter = Formatter::new(&mut file);
    body.format(&mut formatter).map_err(|e| {
        if let hcl::Error::Parse(e) = e {
            Error::from_hcl(e, path.clone())
        } else {
            Error::new(e.to_string())
        }
    })?;

    Ok(())
}
