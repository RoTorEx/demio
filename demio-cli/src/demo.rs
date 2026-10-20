use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Demo {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "root")]
    pub workspace: String,
    #[serde(default)]
    pub adapter: Vec<String>,
    #[serde(default)]
    pub session_seconds: Option<u64>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    pub scenarios: Vec<PathBuf>,
}
fn root() -> String {
    ".".into()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DemoInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub manifest: PathBuf,
    pub folder: PathBuf,
}
impl Demo {
    pub fn load(path: &Path) -> Result<Self> {
        let demo: Self = serde_json::from_slice(
            &fs::read(path).with_context(|| format!("Read demo {}", path.display()))?,
        )
        .context("Invalid demo JSON")?;
        crate::registry::validate_id(&demo.id)?;
        if demo.schema_version != 1
            || demo.name.trim().is_empty()
            || demo.workspace.trim().is_empty()
            || demo.scenarios.is_empty()
        {
            bail!("Demo requires schema_version 1, name, workspace and at least one scenario");
        }
        if demo
            .session_seconds
            .is_some_and(|seconds| !(1..=86400).contains(&seconds))
        {
            bail!("Demo session_seconds must be 1..86400");
        }
        Ok(demo)
    }
    pub fn scenario_paths(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let folder = path
            .parent()
            .context("Demo has no folder")?
            .canonicalize()?;
        let mut paths = Vec::new();
        for relative in &self.scenarios {
            let scenario = folder
                .join(relative)
                .canonicalize()
                .with_context(|| format!("Missing scenario {}", relative.display()))?;
            if relative.is_absolute() || !scenario.starts_with(&folder) || paths.contains(&scenario)
            {
                bail!("Demo scenarios must be distinct files inside the demo folder");
            }
            paths.push(scenario);
        }
        Ok(paths)
    }
    pub fn info(&self, path: &Path) -> Result<DemoInfo> {
        let manifest = path.canonicalize()?;
        Ok(DemoInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            folder: manifest.parent().context("Demo has no folder")?.into(),
            manifest,
        })
    }
}

pub fn is_demo(path: &Path) -> Result<bool> {
    let document: Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(document.get("scenarios").is_some())
}

pub fn scenario_document(path: &Path) -> Result<Value> {
    let mut document: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("Read {}", path.display()))?,
    )
    .context("Invalid scenario JSON")?;
    if let Some(reference) = document.get("demo").and_then(Value::as_str) {
        let demo_path = path
            .parent()
            .context("Scenario has no folder")?
            .join(reference)
            .canonicalize()
            .context("Demo manifest does not exist")?;
        let demo = Demo::load(&demo_path)?;
        let scenario_path = path.canonicalize()?;
        let demo_folder = demo_path.parent().context("Demo has no folder")?;
        if !scenario_path.starts_with(demo_folder)
            || !demo.scenarios.iter().any(|relative| {
                !relative.is_absolute()
                    && demo_folder.join(relative).canonicalize().ok().as_ref()
                        == Some(&scenario_path)
            })
        {
            bail!("Scenario is not listed in its demo manifest");
        }
        let folder = demo_path.parent().context("Demo has no folder")?;
        let mut inherited = serde_json::json!({
            "project": demo.project,
            "workspace": folder.join(demo.workspace),
            "adapter": demo.adapter,
        });
        if let Some(seconds) = demo.session_seconds {
            inherited["session_seconds"] = seconds.into();
        }
        for (key, value) in inherited.as_object().context("Invalid demo defaults")? {
            if document.get(key).is_none() {
                document[key] = value.clone();
            }
        }
        let mut parameters = demo.parameters;
        if let Some(value) = document.get("parameters") {
            let overrides: BTreeMap<String, Value> = serde_json::from_value(value.clone())
                .context("Scenario parameters must be an object")?;
            parameters.extend(overrides);
        }
        document["parameters"] = serde_json::to_value(parameters)?;
        document["demo"] = serde_json::to_value(demo_path)?;
    }
    Ok(document)
}
