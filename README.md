# Demio

One Rust CLI remote for project demos. Register a workflow, choose its stable ID,
and advance at your speaking pace. Project adapters drive the application; Demio
owns sequencing, timeouts, controls, and speaker notes.

The presenter speaks. Demio does not synthesize speech.

## Install

The first release supports **macOS 14+ on Apple Silicon**. Python 3 is needed by
the installer to verify the archive; adapters choose their own dependencies.

```sh
curl -fsSL https://github.com/RoTorEx/demio/releases/latest/download/install.sh -o /tmp/demio-install.sh
sh /tmp/demio-install.sh
rm /tmp/demio-install.sh
demio --version
```

Installation uses `~/.x-cli-demio/bin`, adds it to `.zshrc` once, and preserves
configuration. Open a new terminal after installation. `demio update` installs
the latest release after checking the archive checksum and both native versions.
Old version directories remain available for rollback; remove unused directories
under `~/.x-cli-demio/versions` when no session uses them.

## Channels

```sh
demio register /path/to/project/demo/channel.json
demio list
demio list --json
demio doctor browser-demo
demio run browser-demo
```

`list` shows IDs and names; `run` takes an exact ID, independent of list order.
Run `demio` without arguments for the interactive selector. Registration stores a
reference to the manifest, so project-owned workflow changes are read on launch.
Use `register FILE --replace` to explicitly replace an ID registered elsewhere.
`unregister ID` removes only the registry entry.

## Controls and speaker notes

Two independent floating windows appear during an interactive session:

- A compact movable gray glass remote containing only **Back** and **Next**.
- A movable, resizable speaker notes window showing the step, role, status, and
  text you read aloud. Long text scrolls; resizing changes its wrapping.

F9 advances; F7 previews the previous completed step. Depending on your keyboard
settings, use Fn+F9 / Fn+F7. In the terminal, Enter advances and q / Ctrl+C exits.
There is no hide/show button or F1 toggle. Exit from the terminal to close both
windows and clean up the session. Use `--no-overlay` for terminal controls only.

Back is a **view**, not undo. Moving forward through completed steps previews
without repeating writes. Next is blocked while working or after an error.
Timeouts report an unknown outcome and never trigger automatic retries.

The windows can appear in a full-display recording. Position them outside the
recorded region or record only the application window. This version does not
promise capture exclusion and does not include recording or a phone server.

## Browser example

The example is a real local browser walkthrough: smooth scrolling, paced field
entry, requester/approver roles, submission, and approval. It makes no AI calls
and changes no external services.

```sh
demio init ./my-demo
cd my-demo
python3 -m venv .venv
.venv/bin/python -m pip install playwright==1.63.0
PLAYWRIGHT_SKIP_BROWSER_GC=1 .venv/bin/python -m playwright install chromium
demio register channel.json
demio doctor browser-demo
demio run browser-demo
```

An agent can use the [adapter contract](docs/contracts/adapter.md) to prepare a
workflow for any project. Preparation is project-specific: Demio does not infer
credentials, permission to mutate external services, or a paid model budget.

## Configuration and interface

Like the neighboring personal CLIs, Demio provides `help`, `version`, `update`,
`setup-config`, `config`, `themes`, and `theme NAME`. Themes: calm, classic,
forest, sunset, ocean, midnight. Themes affect the terminal; both native windows
keep the neutral glass appearance.

Config: `$DEMIO_CONFIG`, or `$XDG_CONFIG_HOME/demio/config.toml`, defaulting to
`~/.config/demio/config.toml`. The registry and theme live there. A minimal last
session journal lives in `$XDG_STATE_HOME/demio/last-session.json` (default
`~/.local/state`); it does not resume a stopped session. Runtime files and browser
profiles are removed when the owned session ends.

## Development

Requires Rust 1.85+ and the macOS Command Line Tools. CLI, configuration, protocol,
and step sequencing are Rust. A small Swift/AppKit helper supplies the native
macOS windows and global F7/F9 keys; it performs no project actions.

```sh
make install
make build
make check
make smoke-install
make install-local
```

Builds, packages, and checks use `~/construction_side/demio/`; override
`CARGO_TARGET_DIR` if needed. `make check` checks Rust formatting, clippy, tests,
native compilation, actual CLI behavior, and script syntax without rewriting
source. `make fmt` formats Rust explicitly.

Release from a clean committed main: `make release`, enter the exact version at
its prompt, then `make release-push`. The GitHub workflow builds and verifies the
Apple Silicon bundle before publishing its archive, checksum, and installer.
The project includes Vibecoding Kernel 1.3.5 under `.vibe/kernel`.

MIT licensed.
