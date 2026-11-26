use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset, Utc};
use chrono_tz::Europe::Warsaw;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const THEMES: [&str; 6] = ["calm", "classic", "forest", "sunset", "ocean", "midnight"];
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub role: String,
    pub say: String,
    /// What to wait for before pressing F9, and why. Shown in red; optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub wait: String,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default = "step_timeout")]
    pub timeout_seconds: u64,
}
fn step_timeout() -> u64 {
    120
}
fn session_timeout() -> u64 {
    7200
}
fn default_root() -> String {
    ".".into()
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScenarioType {
    Browser,
    System,
}

impl ScenarioType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Browser => "Browser",
            Self::System => "System",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenBasis {
    Measured,
    Allocated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentTokens {
    pub input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub basis: TokenBasis,
}

impl DevelopmentTokens {
    pub fn basis_label(self) -> &'static str {
        match self.basis {
            TokenBasis::Measured => "measured",
            TokenBasis::Allocated => "allocated",
        }
    }

    pub fn total(self) -> u128 {
        u128::from(self.input) + u128::from(self.output)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demo: Option<PathBuf>,
    #[serde(skip)]
    demo_info: Option<crate::demo::DemoInfo>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub scenario_type: Option<ScenarioType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub development_tokens: Option<DevelopmentTokens>,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub intro: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<FixedOffset>>,
    #[serde(default = "default_root")]
    pub workspace: String,
    pub adapter: Vec<String>,
    #[serde(default = "session_timeout")]
    pub session_seconds: u64,
    pub steps: Vec<Step>,
}
impl Channel {
    pub fn load(path: &Path) -> Result<Self> {
        let source = path
            .canonicalize()
            .with_context(|| format!("Read {}", path.display()))?;
        Self::load_canonical(&source)
    }
    fn load_canonical(source: &Path) -> Result<Self> {
        let (document, demo_info) = crate::demo::scenario_document(source)?;
        let mut channel: Self =
            serde_json::from_value(document).context("Invalid scenario JSON")?;
        channel.demo_info = demo_info;
        channel.validate()?;
        Ok(channel)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("Unsupported schema_version; expected 1");
        }
        validate_id(&self.id)?;
        if self
            .development_tokens
            .is_some_and(|tokens| tokens.cached_input > tokens.input)
        {
            bail!("development_tokens.cached_input cannot exceed input");
        }
        if self.name.trim().is_empty() || self.workspace.trim().is_empty() {
            bail!("Channel name and workspace must not be empty");
        }
        if self.adapter.is_empty() || self.adapter[0].trim().is_empty() {
            bail!("adapter must be a nonempty argument array");
        }
        if !(1..=86400).contains(&self.session_seconds) {
            bail!("session_seconds must be 1..86400");
        }
        if self.steps.is_empty() {
            bail!("Channel must contain at least one step");
        }
        if !self.intro.trim().is_empty() && self.intro.trim() == self.steps[0].say.trim() {
            bail!("The scenario introduction must differ from the first step's narration");
        }
        let mut ids = std::collections::HashSet::new();
        for step in &self.steps {
            validate_id(&step.id)?;
            if !ids.insert(&step.id) {
                bail!("Duplicate step ID: {}", step.id);
            }
            if step.title.trim().is_empty() || step.role.trim().is_empty() {
                bail!("Step title and role must not be empty");
            }
            if !(1..=3600).contains(&step.timeout_seconds) {
                bail!("Step timeout_seconds must be 1..3600");
            }
        }
        Ok(())
    }
    pub fn introduction(&self) -> &str {
        for text in [&self.intro, &self.description] {
            if !text.trim().is_empty() && text.trim() != self.steps[0].say.trim() {
                return text;
            }
        }
        "Introduce the scenario. Begin setup when you are ready."
    }
    pub fn project_name(&self, manifest: &Path) -> String {
        if !self.project.trim().is_empty() {
            return self.project.trim().into();
        }
        self.workspace_path(manifest)
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "Unspecified project".into())
    }
    pub fn display_name(&self, manifest: &Path) -> String {
        prefixed_name(&self.project_name(manifest), &self.name)
    }
    pub fn workspace_path(&self, manifest: &Path) -> Result<PathBuf> {
        let source = manifest.canonicalize().context("Manifest does not exist")?;
        self.workspace_from_source(&source)
    }
    fn workspace_from_source(&self, source: &Path) -> Result<PathBuf> {
        let root = source
            .parent()
            .context("Manifest has no parent directory")?
            .join(&self.workspace);
        let root = root.canonicalize().context("Workspace does not exist")?;
        if !root.is_dir() {
            bail!("Workspace must be a directory");
        }
        Ok(root)
    }
}
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    {
        bail!(
            "IDs must be 1..64 ASCII letters, digits, hyphens or underscores, starting with a letter or digit"
        );
    }
    Ok(())
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demo: Option<crate::demo::DemoInfo>,
    pub name: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub scenario_type: Option<ScenarioType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub development_tokens: Option<DevelopmentTokens>,
    pub manifest: PathBuf,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: Option<DateTime<FixedOffset>>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub overlay: toml::Table,
    pub channels: BTreeMap<String, Entry>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "calm".into(),
            overlay: toml::Table::new(),
            channels: BTreeMap::new(),
        }
    }
}
pub fn home() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME")
        .context("HOME is unavailable")?
        .into())
}
pub fn product_home() -> Result<PathBuf> {
    Ok(std::env::var_os("DEMIO_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".toolbelt/demio")))
}
pub fn config_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("DEMIO_CONFIG") {
        return Ok(path.into());
    }
    Ok(product_home()?.join("config.toml"))
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".demio-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::write(&tmp, bytes)?;
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}
pub fn now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&Warsaw).fixed_offset()
}
pub fn prefixed_name(project: &str, name: &str) -> String {
    let prefix = format!("{project} · ");
    if name.starts_with(&prefix) {
        name.into()
    } else {
        format!("{prefix}{name}")
    }
}
#[derive(Serialize)]
pub struct Listing {
    pub demo: Option<crate::demo::DemoInfo>,
    pub id: String,
    pub name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub scenario_type: Option<ScenarioType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub development_tokens: Option<DevelopmentTokens>,
    pub display_name: String,
    pub project: String,
    pub description: String,
    pub created_at: Option<DateTime<FixedOffset>>,
    pub manifest: PathBuf,
    pub available: bool,
}
impl Listing {
    pub fn created_label(&self) -> String {
        self.created_at
            .map(|date| {
                date.with_timezone(&Warsaw)
                    .format("%Y-%m-%d %H:%M:%S %Z (UTC%:z)")
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown creation time".into())
    }
}
impl Config {
    pub fn load() -> Result<Self> {
        crate::overlay::migrate()?;
        let path = config_path()?;
        let result: Self = if path.exists() {
            toml::from_str(&fs::read_to_string(path)?).context("Invalid Demio config")?
        } else {
            Self::default()
        };
        if !THEMES.contains(&result.theme.as_str()) {
            bail!("Unknown theme in config: {}", result.theme);
        }
        Ok(result)
    }
    pub fn save(&self) -> Result<()> {
        atomic_write(&config_path()?, toml::to_string_pretty(self)?.as_bytes())
    }
    pub fn resolve(&self, id: &str) -> Result<(Channel, PathBuf)> {
        let entry = self
            .channels
            .get(id)
            .with_context(|| format!("Unknown channel ID: {id}. Run demio list."))?;
        let path = entry
            .manifest
            .canonicalize()
            .context("Manifest does not exist")?;
        let channel = Channel::load_canonical(&path)?;
        if channel.id != id {
            bail!(
                "Registered ID no longer matches manifest; register the updated channel explicitly"
            );
        }
        channel.workspace_from_source(&path)?;
        Ok((channel, path))
    }
    pub fn listing(&self) -> Vec<Listing> {
        let mut rows: Vec<_> = self
            .channels
            .iter()
            .map(|(id, entry)| {
                let resolved = self.resolve(id);
                let (name, project, description, created_at, scenario_type, development_tokens) =
                    match &resolved {
                        Ok((channel, path)) => (
                            channel.name.clone(),
                            channel.project_name(path),
                            channel.description.clone(),
                            channel.created_at.or(entry.created_at),
                            channel.scenario_type,
                            channel.development_tokens,
                        ),
                        Err(_) => (
                            entry.name.clone(),
                            if entry.project.is_empty() {
                                "Unspecified project".into()
                            } else {
                                entry.project.clone()
                            },
                            entry.description.clone(),
                            entry.created_at,
                            entry.scenario_type,
                            entry.development_tokens,
                        ),
                    };
                let created_at = created_at.or_else(|| {
                    fs::metadata(&entry.manifest)
                        .ok()?
                        .created()
                        .ok()
                        .map(|time| DateTime::<Utc>::from(time).fixed_offset())
                });
                let demo = resolved
                    .as_ref()
                    .ok()
                    .and_then(|(channel, _)| channel.demo_info.clone())
                    .or_else(|| entry.demo.clone());
                Listing {
                    demo,
                    id: id.clone(),
                    display_name: prefixed_name(&project, &name),
                    name,
                    scenario_type,
                    development_tokens,
                    project,
                    description,
                    created_at,
                    manifest: entry.manifest.clone(),
                    available: resolved.is_ok(),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        rows
    }
    pub fn register(&mut self, path: &Path, replace: bool) -> Result<String> {
        Ok(self
            .register_files(&[path.canonicalize()?], replace)?
            .remove(0))
    }
    pub fn register_demo(&mut self, path: &Path, replace: bool) -> Result<Vec<String>> {
        let path = path.canonicalize()?;
        let demo = crate::demo::Demo::load(&path)?;
        let paths = demo.scenario_paths(&path)?;
        for scenario in &paths {
            if Channel::load(scenario)?.demo.as_ref() != Some(&path) {
                bail!("Every scenario must reference its owning demo manifest");
            }
        }
        self.register_files(&paths, replace)
    }
    fn register_files(&mut self, paths: &[PathBuf], replace: bool) -> Result<Vec<String>> {
        let mut prepared = Vec::new();
        let mut ids = std::collections::HashSet::new();
        // Validate the complete batch before changing the registry or timestamps.
        for manifest in paths {
            let mut channel = Channel::load_canonical(manifest)?;
            channel.workspace_from_source(manifest)?;
            if !ids.insert(channel.id.clone()) {
                bail!("Duplicate scenario ID: {}", channel.id);
            }
            if let Some(old) = self.channels.get(&channel.id) {
                if old.manifest != *manifest && !replace {
                    bail!(
                        "ID {} is registered elsewhere; use --replace to change it",
                        channel.id
                    );
                }
            }
            let mut write = None;
            if channel.created_at.is_none() {
                channel.created_at = self
                    .channels
                    .get(&channel.id)
                    .filter(|old| old.manifest == *manifest)
                    .and_then(|old| old.created_at)
                    .or_else(|| Some(now()));
                let mut document: serde_json::Value = serde_json::from_slice(&fs::read(manifest)?)?;
                document["created_at"] = serde_json::to_value(channel.created_at)?;
                let mut bytes = serde_json::to_vec_pretty(&document)?;
                bytes.push(b'\n');
                write = Some(bytes);
            }
            let project = channel.project_name(manifest);
            let entry = Entry {
                demo: channel.demo_info,
                scenario_type: channel.scenario_type,
                development_tokens: channel.development_tokens,
                project,
                description: channel.description,
                created_at: channel.created_at,
                name: channel.name,
                manifest: manifest.clone(),
            };
            prepared.push((channel.id, entry, write));
        }
        for (_, entry, write) in &prepared {
            if let Some(bytes) = write {
                atomic_write(&entry.manifest, bytes)?;
            }
        }
        let ids = prepared.iter().map(|(id, _, _)| id.clone()).collect();
        for (id, entry, _) in prepared {
            self.channels.insert(id, entry);
        }
        self.save()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn warsaw_creation_labels_follow_the_event_date() {
        let mut row = Listing {
            demo: None,
            id: "demo".into(),
            name: "Example".into(),
            scenario_type: None,
            development_tokens: None,
            display_name: "Project · Example".into(),
            project: "Project".into(),
            description: "Description".into(),
            created_at: Some(DateTime::parse_from_rfc3339("2026-07-01T12:00:00Z").unwrap()),
            manifest: "channel.json".into(),
            available: true,
        };
        assert!(row.created_label().contains("14:00:00 CEST (UTC+02:00)"));
        row.created_at = Some(DateTime::parse_from_rfc3339("2026-01-01T12:00:00Z").unwrap());
        assert!(row.created_label().contains("13:00:00 CET (UTC+01:00)"));
    }
    #[test]
    fn project_prefix_is_not_duplicated() {
        assert_eq!(prefixed_name("Project", "Example"), "Project · Example");
        assert_eq!(
            prefixed_name("Project", "Project · Example"),
            "Project · Example"
        );
    }
}
