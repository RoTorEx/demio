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

## Demos and scenarios

A **demo** is one folder with a shared environment, adapters and assets. A
**scenario** is one route through that demo: its own steps, narration and input
parameters. Several scenarios reuse the same components.

```text
identity-graph/
  AGENTS.md
  README.md
  demo.json
  shared/
    adapter.py
  scenarios/
    investigation/scenario.json
    role-change/scenario.json
    onboarding/scenario.json
```

```sh
demio init ./my-demo
demio validate ./my-demo/demo.json
demio register ./my-demo/demo.json
demio list
demio
```

Init scaffolds two working browser scenarios, shared components, a brief and
local agent instructions. Read those instructions and agree the real demo brief
before adapting it. Registration accepts demo.json (all declared scenarios) or a
single scenario JSON. Run and doctor use a stable **scenario ID** printed by list.
Legacy standalone manifests remain supported.

The two-pane TUI shows demo headers with their scenarios on the left and
separate demo/scenario details on the right. Dates retain the year; names use
` | `, with colors following GTD roles. ↑/↓ or j/k selects a scenario; Enter runs
it; PgUp/PgDn scrolls details; e opens the owning demo root and selected scenario
in VS Code; f reveals the scenario in Finder; q/Esc quits. Panes stay side by
side. Non-terminal output remains a plain list; list --json includes demo
identity and paths. Empty/unavailable states stay visible without running code.

`demo.json` owns shared defaults. Scenario overrides are resolved on launch.
`parameters` merge by key and reach the adapter as JSON in `DEMIO_PARAMETERS`;
`DEMIO_SCENARIO` identifies the selected JSON. New scenarios should reuse shared
actions and vary parameters/narration rather than copy adapters. The scaffold's
AGENTS.md explains boundaries, controls, user agreement and meaningful checks.

Registration stores source paths rather than copying files. Reopen the selector
to refresh metadata after editing. Preserve IDs and creation timestamps; new
scenarios get timestamps on init/registration. Warsaw dates use the event's
CET/CEST offset. Use --replace explicitly when moving an existing ID to another
manifest. Unregister removes only the registration.

See [Editing demos and scenarios](docs/scenarios.md) and the
[adapter contract](docs/contracts/adapter.md) for full examples and path rules.

## Controls and speaker notes

One movable, resizable gray glass presenter window contains **Back**, **Next**,
the current step, role, status, and the text you read aloud. Navigation stays in
the same window as the notes. Long text scrolls and wraps as the window resizes.
Use **A− / A+** to change font size and **Left / Center / Right** for alignment.
These preferences persist in `speaker-notes.json` beside the CLI config.

F9 advances; F7 previews the previous completed step. Depending on your keyboard
settings, use Fn+F9 / Fn+F7. In the terminal, Enter advances and q / Ctrl+C exits.
There is no hide/show button or F1 toggle. Exit from the terminal to close the
window and clean up the session. Use `--no-overlay` for terminal controls only.

Back is a **view**, not undo. Moving forward through completed steps previews
without repeating writes. Next is blocked while working or after an error.
Timeouts report an unknown outcome and never trigger automatic retries.

The window requests capture exclusion (`NSWindow.sharingType = .none`), matching
the original presenter remote. This keeps controls and notes visible to the
presenter while excluding them in recorders that honor that flag. Modern
ScreenCaptureKit recorders, including full-display recording through ⌘⇧5 on newer
macOS versions, can ignore it. Check a short saved recording on the recording Mac
before a demo. If the window appears, position it outside the recorded region
or use a recorder that explicitly excludes Demio. Demio does not include recording
or a phone server.

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
demio register demo.json
demio list
demio
```

An agent can use the [adapter contract](docs/contracts/adapter.md) to prepare a
workflow for any project. Preparation is project-specific: Demio does not infer
credentials, permission to mutate external services, or a paid model budget.

## Configuration and interface

Like the neighboring personal CLIs, Demio provides `help`, `version`, `update`,
`setup-config`, `config`, `themes`, and `theme NAME`. Themes: calm, classic,
forest, sunset, ocean, midnight. Themes affect the terminal; the native window
keeps the neutral glass appearance.

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
