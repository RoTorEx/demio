# Demio

Demio is a product with two components: **demio-cli** (Rust; public command
`demio`) and **demio-overlay** (Swift/AppKit; the native presenter window).
The checkout belongs under `personal/projects/dev-tools/demio`, with a shared version,
package, and release workflow for both components.

```text
demio/
  Cargo.toml
  demio-cli/
    Cargo.toml
    src/
  demio-overlay/
    Overlay.swift
  templates/
  examples/
  scripts/
  tests/
```

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

Installation uses `~/.toolbelt/demio/bin` and adds it to `.zshrc` once. Open a
new terminal after installation. `demio update` verifies the archive checksum
and both native versions, then replaces the current bundle. There is one
installed version; updates preserve configuration, demos and logs. Stop active
demos before updating. Custom installations use `--install-dir PATH` and set
`DEMIO_HOME=PATH` when launching.

```text
~/.toolbelt/demio/
  config.toml           # Global theme and scenario registry
  bin/                  # Current demio + demio-overlay bundle only
  demos/
    identity-graph/
      demo.json         # This demo's configuration stays with its components
      AGENTS.md
      README.md
      shared/
      scenarios/
  logs/                 # Bounded diagnostics, no session resume data
```

The source repository contains product code, public examples and templates.
Personal/customer demos belong in the product home's demos directory. Registering
an existing external demo remains supported. The installer imports old settings
from `~/.config/demio` once when no new settings exist, preserving the originals.
It also replaces the old Demio PATH entry. Move existing demos explicitly, then
register their demo.json with --replace; the installer never moves project files.

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
demio init ~/.toolbelt/demio/demos/my-demo
demio validate ~/.toolbelt/demio/demos/my-demo/demo.json
demio register ~/.toolbelt/demio/demos/my-demo/demo.json
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
These preferences persist in the shared config.toml under `[overlay]`:

```toml
[overlay]
font_size = 22.0
alignment = "left" # left, center, right
```

The native window uses the CLI's config reader/writer, so theme, registry and
other fields survive preference changes. Existing speaker-notes.json settings
migrate automatically on install or the next CLI/window launch. A pre-existing
[overlay] section wins; the old JSON is removed only after a successful migration.
Malformed legacy JSON is preserved with a warning for manual recovery.

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
demio init ~/.toolbelt/demio/demos/my-demo
cd ~/.toolbelt/demio/demos/my-demo
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

Config: `$DEMIO_CONFIG`, otherwise `$DEMIO_HOME/config.toml`, defaulting to
`~/.toolbelt/demio/config.toml`. The registry and theme live at the top level;
each demo's configuration remains in its own demo.json. DEMIO_HOME overrides the
product home for isolated/custom installations. Explicit DEMIO_CONFIG selects the same file
for the CLI and native presenter preferences. XDG config/state defaults are no longer used.

`logs/demio.jsonl` records timestamps, process/scenario IDs, completed step count,
view and status. It excludes narration, parameters and adapter payloads. At 1 MiB
it rotates to one previous log. These are diagnostics, never resumable state.
Session files/browser profiles use the OS temporary directory and are removed on
normal exit; DEMIO_RUNTIME_DIR can override the temporary base. No permanent
state, sessions or backup directories are created. A crash may leave temporary
files; project adapters still own external effects and project runtime data.

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

## Local usage logs

See [usage logging](docs/usage-logging.md) for product events, privacy, paths,
rotation and interpretation limits.
