//! Shared config persistence for the native presenter, owned by the CLI.
use crate::registry::{atomic_write, config_path};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub font_size: f64,
    pub alignment: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: 22.0,
            alignment: "left".into(),
        }
    }
}
impl Settings {
    fn normalized(mut self) -> Self {
        self.font_size = if self.font_size.is_finite() {
            self.font_size.clamp(14.0, 48.0)
        } else {
            22.0
        };
        if !matches!(self.alignment.as_str(), "left" | "center" | "right") {
            self.alignment = "left".into();
        }
        self
    }
}
fn document() -> Result<DocumentMut> {
    let path = config_path()?;
    if path.exists() {
        fs::read_to_string(path)?
            .parse()
            .context("Invalid Demio config")
    } else {
        Ok(DocumentMut::new())
    }
}
fn apply(document: &mut DocumentMut, settings: &Settings) -> Result<()> {
    if !document.contains_key("overlay") {
        document["overlay"] = Item::Table(Table::new());
    }
    let table = document["overlay"]
        .as_table_like_mut()
        .context("overlay must be a config table")?;
    table.insert("font_size", value(settings.font_size));
    table.insert("alignment", value(&settings.alignment));
    Ok(())
}
pub fn migrate() -> Result<()> {
    let config = config_path()?;
    let legacy = config.with_file_name("speaker-notes.json");
    if !legacy.exists() {
        return Ok(());
    }
    let settings = match serde_json::from_slice::<Settings>(&fs::read(&legacy)?) {
        Ok(settings) => settings.normalized(),
        Err(_) => {
            eprintln!("Could not migrate speaker-notes.json; original file preserved.");
            return Ok(());
        }
    };
    let mut document = document()?;
    // A pre-existing common-config section always wins over legacy preferences.
    if !document.contains_key("overlay") {
        apply(&mut document, &settings)?;
        atomic_write(&config, document.to_string().as_bytes())?;
    }
    if !document["overlay"].is_table_like() {
        bail!("overlay must be a config table");
    }
    fs::remove_file(legacy)?;
    Ok(())
}
pub fn command(arguments: &[String]) -> Result<()> {
    migrate()?;
    let mut document = document()?;
    let settings = if arguments.is_empty() {
        Settings {
            font_size: document
                .get("overlay")
                .and_then(|t| t.get("font_size"))
                .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|v| v as f64)))
                .unwrap_or(22.0),
            alignment: document
                .get("overlay")
                .and_then(|t| t.get("alignment"))
                .and_then(Item::as_str)
                .unwrap_or("left")
                .into(),
        }
        .normalized()
    } else {
        if arguments.len() != 2 {
            bail!("Expected font size and alignment");
        }
        let settings = Settings {
            font_size: arguments[0].parse()?,
            alignment: arguments[1].clone(),
        }
        .normalized();
        apply(&mut document, &settings)?;
        atomic_write(&config_path()?, document.to_string().as_bytes())?;
        settings
    };
    println!("{}", serde_json::to_string(&settings)?);
    Ok(())
}
