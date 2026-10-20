mod protocol;
mod registry;
mod run;
mod selector;
use anyhow::{Context, Result, bail};
use registry::{Channel, Config, THEMES, config_path};
use std::{
    fs,
    io::{self, IsTerminal},
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const HELP: &str = "Demio — one remote for project demos\n\nUsage: demio [COMMAND]\n\n  list [--json]                 List projects, descriptions and creation times\n  register FILE [--replace]      Register a project-owned channel manifest\n  unregister ID                 Remove registry entry, preserve project files\n  validate FILE                 Validate channel without executing code\n  doctor ID                     Ask adapter for a read-only readiness check\n  run ID [--no-overlay] [--auto]  Run by exact channel ID\n  init DIRECTORY                Scaffold a browser demo / adapter example\n  setup-config                  Create missing configuration\n  config                        Print configuration and path\n  themes                        List themes\n  theme NAME                    Set terminal theme\n  update                        Install latest verified release\n  help / --help                 Show this help\n  version / --version           Show version\n\nWithout a command: select a channel interactively (list outside a terminal).\nF9 / Enter: next; F7: previous view; q / Ctrl+C: stop.\n";
fn main() {
    if let Err(error) = application() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
fn argument<'a>(args: &'a [String], index: usize, usage: &str) -> Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .with_context(|| format!("Usage: {usage}"))
}
fn application() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    if matches!(command, "help" | "--help" | "-h") {
        print!("{HELP}");
        return Ok(());
    }
    if matches!(command, "version" | "--version" | "-V") {
        println!("demio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    let mut config = Config::load()?;
    match command {
        "" => {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                return list(&config, false);
            }
            if let Some(id) = selector::choose(&config, &cancelled)? {
                let (channel, path) = config.resolve(&id)?;
                run::run(channel, path, &config.theme, false, false, cancelled)?;
            }
        }
        "list" => {
            if args.len() > 2 || (args.len() == 2 && args[1] != "--json") {
                bail!("Usage: demio list [--json]");
            }
            list(&config, args.get(1).is_some())?;
        }
        "register" => {
            if args.len() > 3 || (args.len() == 3 && args[2] != "--replace") {
                bail!("Usage: demio register FILE [--replace]");
            }
            let id = config.register(
                Path::new(argument(&args, 1, "demio register FILE [--replace]")?),
                args.get(2).is_some(),
            )?;
            println!("Registered {id}. Run: demio run {id}");
        }
        "unregister" => {
            let id = argument(&args, 1, "demio unregister ID")?;
            if config.channels.remove(id).is_none() {
                bail!("Unknown channel ID: {id}");
            }
            config.save()?;
            println!("Unregistered {id}; project files preserved");
        }
        "validate" => {
            let path = Path::new(argument(&args, 1, "demio validate FILE")?).canonicalize()?;
            let channel = Channel::load(&path)?;
            channel.workspace_path(&path)?;
            println!(
                "Valid: {} [{}] — {} steps",
                channel.name,
                channel.id,
                channel.steps.len()
            );
        }
        "doctor" => {
            let (channel, path) = config.resolve(argument(&args, 1, "demio doctor ID")?)?;
            run::doctor(&channel, &path, &cancelled)?;
        }
        "run" => {
            let id = argument(&args, 1, "demio run ID [--no-overlay] [--auto]")?;
            for option in args.iter().skip(2) {
                if !matches!(option.as_str(), "--auto" | "--no-overlay") {
                    bail!("Unknown run option: {option}");
                }
            }
            let (channel, path) = config.resolve(id)?;
            run::run(
                channel,
                path,
                &config.theme,
                args.iter().any(|x| x == "--auto"),
                args.iter().any(|x| x == "--no-overlay"),
                cancelled,
            )?;
        }
        "setup-config" => {
            if !config_path()?.exists() {
                config.save()?;
            }
            println!("{}", config_path()?.display());
        }
        "config" => {
            println!(
                "Config: {}\n{}",
                config_path()?.display(),
                toml::to_string_pretty(&config)?
            );
        }
        "themes" => {
            for theme in THEMES {
                println!("{theme}{}", if config.theme == theme { " *" } else { "" });
            }
        }
        "theme" => {
            let theme = argument(&args, 1, "demio theme NAME")?;
            if !THEMES.contains(&theme) {
                bail!("Unknown theme; run demio themes");
            }
            config.theme = theme.into();
            config.save()?;
            println!("Theme: {theme}");
        }
        "init" => scaffold(Path::new(argument(&args, 1, "demio init DIRECTORY")?))?,
        "update" => {
            let script = std::env::current_exe()?
                .parent()
                .context("Executable parent unavailable")?
                .join("demio-install.sh");
            if !script.is_file() {
                bail!("Installer missing; reinstall from the release installer");
            }
            let native_dir = script.parent().context("Install directory unavailable")?;
            let parent = native_dir.parent().context("Install root unavailable")?;
            let prefix = if parent.file_name().is_some_and(|name| name == "versions") {
                parent.parent().context("Install root unavailable")?
            } else {
                parent
            };
            let status = Command::new("sh")
                .arg(script.clone())
                .args([
                    "--install-dir",
                    &prefix.to_string_lossy(),
                    "--no-path-update",
                ])
                .status()?;
            if !status.success() {
                bail!("Update failed; previous executable was preserved until verification");
            }
        }
        _ => bail!("Unknown command: {command}. Run demio help."),
    }
    Ok(())
}
fn list(config: &Config, json: bool) -> Result<()> {
    let entries = config.listing();
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        println!(
            "No channels registered.\nCreate one: demio init ./my-demo\nRegister it: demio register ./my-demo/channel.json"
        );
    } else {
        println!("{:<24} NAME / PROJECT", "ID");
        for entry in entries {
            println!(
                "{:<24} {}{}",
                entry.id,
                entry.display_name,
                if entry.available {
                    ""
                } else {
                    " [unavailable]"
                }
            );
            println!(
                "                         Created: {} Warsaw",
                entry.created_label()
            );
            println!(
                "                         {}\n",
                if entry.description.trim().is_empty() {
                    "No description provided"
                } else {
                    &entry.description
                }
            );
        }
    }
    Ok(())
}
fn scaffold(path: &Path) -> Result<()> {
    for name in ["channel.json", "adapter.py", "index.html"] {
        if path.join(name).exists() {
            bail!("Refusing to overwrite {}", path.join(name).display());
        }
    }
    fs::create_dir_all(path)?;
    let mut channel: serde_json::Value =
        serde_json::from_str(include_str!("../examples/browser/channel.json"))?;
    channel["created_at"] = serde_json::to_value(registry::now())?;
    fs::write(
        path.join("channel.json"),
        format!("{}\n", serde_json::to_string_pretty(&channel)?),
    )?;
    fs::write(
        path.join("adapter.py"),
        include_str!("../examples/browser/adapter.py"),
    )?;
    fs::write(
        path.join("index.html"),
        include_str!("../examples/browser/index.html"),
    )?;
    println!(
        "Created {}\nNext: prepare a Python venv with Playwright, then demio register {}/channel.json.\nSee https://github.com/RoTorEx/demio#browser-example",
        path.display(),
        path.display()
    );
    Ok(())
}
