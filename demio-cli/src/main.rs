mod demo;
mod overlay;
mod protocol;
mod registry;
mod run;
mod selector;
mod usage;
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
const HELP: &str = "Demio — one remote for project demos\n\nUsage: demio [COMMAND]\n\n  list [--json]                 List demos/scenarios and creation times\n  register FILE [--replace]      Register a demo or scenario manifest\n  unregister ID                 Remove registry entry, preserve project files\n  validate FILE                 Validate demo/scenario without executing code\n  doctor ID                     Ask adapter for a read-only readiness check\n  run ID [--no-overlay] [--auto|--dry-run]  Run by exact scenario ID\n                                --dry-run: narration only, nothing executed\n  init DIRECTORY                Scaffold a demo, scenarios and instructions\n  setup-config                  Create missing configuration\n  config                        Print configuration and path\n  themes                        List themes\n  theme NAME                    Set terminal theme\n  update                        Install latest verified release\n  help / --help                 Show this help\n  version / --version           Show version\n\nWithout a command: select a scenario interactively (list outside a terminal);\nEnter opens the presenter and d opens a rehearsal.\ns  Start scenario    F9  Next    F7  Previous view\nc  Confirm completion    q / Ctrl+C  Stop\n";
fn main() {
    if let Ok(home) = registry::product_home() {
        usage::init("demio", home.join("logs/demio.jsonl"));
    }
    let op = usage::Operation::start("command");
    let result = application();
    op.finish(
        if result.is_ok() { "success" } else { "error" },
        serde_json::json!({}),
    );
    if let Err(error) = result {
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
    usage::event(
        "command_selected",
        "started",
        serde_json::json!({"mode":match command {
            ""=>"default", "help"|"--help"|"-h"=>"help", "version"|"--version"|"-V"=>"version",
            "list"|"register"|"unregister"|"validate"|"doctor"|"run"|"init"|"setup-config"|"config"|"themes"|"theme"|"update"=>command,
            "--overlay-settings"=>"overlay_settings", _=>"unknown"
        }}),
    );
    if matches!(command, "help" | "--help" | "-h") {
        print!("{HELP}");
        return Ok(());
    }
    if matches!(command, "version" | "--version" | "-V") {
        println!("demio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if command == "--overlay-settings" {
        return overlay::command(&args[1..]);
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
            if let Some((id, dry)) = selector::choose(&config, &cancelled)? {
                let (channel, path) = config.resolve(&id)?;
                run::run(channel, path, &config.theme, false, false, dry, cancelled)?;
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
            let path = Path::new(argument(&args, 1, "demio register FILE [--replace]")?);
            let ids = if demo::is_demo(path)? {
                config.register_demo(path, args.get(2).is_some())?
            } else {
                vec![config.register(path, args.get(2).is_some())?]
            };
            for id in ids {
                println!("Registered {id}. Run: demio run {id}");
            }
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
            let paths = if demo::is_demo(&path)? {
                demo::Demo::load(&path)?.scenario_paths(&path)?
            } else {
                vec![path]
            };
            for path in paths {
                let channel = Channel::load(&path)?;
                channel.workspace_path(&path)?;
                println!(
                    "Valid scenario: {} [{}] — {} steps",
                    channel.name,
                    channel.id,
                    channel.steps.len()
                );
            }
        }
        "doctor" => {
            let (channel, path) = config.resolve(argument(&args, 1, "demio doctor ID")?)?;
            run::doctor(&channel, &path, &cancelled)?;
        }
        "run" => {
            let id = argument(&args, 1, "demio run ID [--no-overlay] [--auto|--dry-run]")?;
            for option in args.iter().skip(2) {
                if !matches!(option.as_str(), "--auto" | "--no-overlay" | "--dry-run") {
                    bail!("Unknown run option: {option}");
                }
            }
            let auto = args.iter().any(|x| x == "--auto");
            let dry = args.iter().any(|x| x == "--dry-run");
            if auto && dry {
                bail!("--auto executes every step and --dry-run executes none; choose one");
            }
            let (channel, path) = config.resolve(id)?;
            run::run(
                channel,
                path,
                &config.theme,
                auto,
                args.iter().any(|x| x == "--no-overlay"),
                dry,
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
            let legacy = prefix == registry::home()?.join(".x-cli-demio");
            let destination = if legacy {
                registry::product_home()?
            } else {
                prefix.to_path_buf()
            };
            let mut installer = Command::new("sh");
            installer
                .arg(script.clone())
                .arg("--install-dir")
                .arg(destination);
            if !legacy {
                installer.arg("--no-path-update");
            }
            let status = installer.status()?;
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
        println!("{:<24} DEMO / SCENARIO", "ID");
        for entry in entries {
            println!(
                "{:<24} {}{}",
                entry.id,
                entry
                    .demo
                    .as_ref()
                    .map(|demo| format!("{} | {}", demo.name, entry.name))
                    .unwrap_or_else(|| entry.display_name.clone()),
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
    if path.exists() && fs::read_dir(path)?.next().is_some() {
        bail!(
            "Refusing to overwrite nonempty demo folder {}",
            path.display()
        );
    }
    fn id() -> Result<String> {
        use std::io::Read;
        let mut bytes = [0u8; 6];
        fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        Ok(bytes
            .iter()
            .map(|byte| alphabet[*byte as usize % alphabet.len()] as char)
            .collect())
    }
    let templates = [
        (
            "navigation",
            include_str!("../../examples/browser/scenarios/navigation.json"),
        ),
        (
            "waiting",
            include_str!("../../examples/browser/scenarios/waiting.json"),
        ),
        (
            "timeout",
            include_str!("../../examples/browser/scenarios/timeout.json"),
        ),
        (
            "recovery",
            include_str!("../../examples/browser/scenarios/recovery.json"),
        ),
        ("mac-file", include_str!("../../examples/mac/scenario.json")),
    ];
    fs::create_dir_all(path.join("shared"))?;
    let mut ids = std::collections::HashSet::new();
    let mut scenarios = Vec::new();
    let demo_id = id()?;
    ids.insert(demo_id.clone());
    for (variant, template) in templates {
        let mut identity = id()?;
        while !ids.insert(identity.clone()) {
            identity = id()?;
        }
        let mut scenario: serde_json::Value = serde_json::from_str(template)?;
        scenario["id"] = identity.into();
        scenario["demo"] = "../../demo.json".into();
        scenario["created_at"] = serde_json::to_value(registry::now())?;
        let folder = format!("scenarios/{variant}");
        fs::create_dir_all(path.join(&folder))?;
        let relative = format!("{folder}/scenario.json");
        fs::write(
            path.join(&relative),
            format!("{}\n", serde_json::to_string_pretty(&scenario)?),
        )?;
        scenarios.push(relative);
    }
    let demo = serde_json::json!({
        "schema_version":1,
        "id":demo_id,
        "name":"Demio rehearsals",
        "project":"Demio",
        "description":"Rehearse browser actions, presenter controls and a native Mac file walkthrough.",
        "workspace":".",
        "adapter":[".venv/bin/python","-u","shared/web.py"],
        "scenarios":scenarios
    });
    fs::write(
        path.join("demo.json"),
        format!("{}\n", serde_json::to_string_pretty(&demo)?),
    )?;
    for (name, content) in [
        ("AGENTS.md", include_str!("../../templates/demo/AGENTS.md")),
        ("README.md", include_str!("../../templates/demo/README.md")),
        (
            "shared/web.py",
            include_str!("../../examples/browser/web.py"),
        ),
        (
            "shared/mac_file.py",
            include_str!("../../examples/mac/mac_file.py"),
        ),
    ] {
        fs::write(path.join(name), content)?;
    }
    println!(
        "Created demo {} with browser and Mac rehearsals. See README.md for setup. Register with demio register {}/demo.json",
        path.display(),
        path.display()
    );
    Ok(())
}
