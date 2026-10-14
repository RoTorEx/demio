use anyhow::{Context, Result, bail};
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default = "default_root")]
    pub workspace: String,
    pub adapter: Vec<String>,
    #[serde(default = "session_timeout")]
    pub session_seconds: u64,
    pub steps: Vec<Step>,
}
impl Channel {
    pub fn load(path: &Path) -> Result<Self> {
        let channel: Self = serde_json::from_slice(
            &fs::read(path).with_context(|| format!("Read {}", path.display()))?,
        )
        .context("Invalid channel JSON")?;
        channel.validate()?;
        Ok(channel)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!("Unsupported schema_version; expected 1");
        }
        validate_id(&self.id)?;
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
    pub fn workspace_path(&self, manifest: &Path) -> Result<PathBuf> {
        let root = manifest
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
    pub name: String,
    pub manifest: PathBuf,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub channels: BTreeMap<String, Entry>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "calm".into(),
            channels: BTreeMap::new(),
        }
    }
}
pub fn home() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME")
        .context("HOME is unavailable")?
        .into())
}
pub fn config_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("DEMIO_CONFIG") {
        return Ok(path.into());
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".config"));
    Ok(base.join("demio/config.toml"))
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
impl Config {
    pub fn load() -> Result<Self> {
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
        let channel = Channel::load(&entry.manifest)?;
        if channel.id != id {
            bail!(
                "Registered ID no longer matches manifest; register the updated channel explicitly"
            );
        }
        channel.workspace_path(&entry.manifest)?;
        Ok((channel, entry.manifest.clone()))
    }
    pub fn register(&mut self, path: &Path, replace: bool) -> Result<String> {
        let manifest = path.canonicalize().context("Manifest does not exist")?;
        let channel = Channel::load(&manifest)?;
        channel.workspace_path(&manifest)?;
        if let Some(old) = self.channels.get(&channel.id) {
            if old.manifest != manifest && !replace {
                bail!(
                    "ID {} is registered elsewhere; use --replace to change it",
                    channel.id
                );
            }
        }
        self.channels.insert(
            channel.id.clone(),
            Entry {
                name: channel.name,
                manifest,
            },
        );
        self.save()?;
        Ok(channel.id)
    }
}
