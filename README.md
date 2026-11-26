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

Init scaffolds browser and native Mac rehearsals, shared components and local
agent instructions. Read those instructions and agree the real demo brief
before adapting it. Registration accepts demo.json (all declared scenarios) or a
single scenario JSON. Run and doctor use a stable **scenario ID** printed by list.
Legacy standalone manifests remain supported.

The left pane shows a demo tree with ID, short creation date (DD.MM.YY) and
scenario name. Legacy IDs show their first six characters and an ellipsis.
The right pane starts with identity metadata, then Description and Steps.
Introduction is step zero. Development tokens, run time and source paths
appear below the steps. The wheel scrolls the pane under
the pointer. ↑/↓ or j/k changes selection. Enter opens the presenter at step zero,
and d opens a dry rehearsal. PgUp/PgDn scrolls the right pane. Press e to open the
demo root and scenario in VS Code, f to reveal
the scenario in Finder, or q/Esc to quit. Non-terminal output remains a plain
list. `demio list --json` includes stable IDs and source paths.

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

Pressing Enter on a scenario opens one movable presenter window on step **0**, the
introduction. **Start scenario** opens the adapter and prepares its windows and
data. Once preparation succeeds the presenter shows the distinct first step.
**Forward** starts that step. **Back**, **Start** and **Forward** stay on the
first row. The fixed-size state button stays beside **Stop** on the second row.
The scenario name and ID appear above the step and its position, such as
**Introduction (0 of 6)**. The current status has its own line.
**Info** and **Transcript** wrap and scroll as the window resizes.
The panel opens with a 360 × 288 pt content area and 12 pt body text by
default. Use the gear to change the transcript size or alignment. Hover over
**ⓘ** for keyboard shortcuts. Split transcript topics with a blank line and
put short cues for the presenter in angle brackets, such as `<Pause here>`.
The shared config.toml stores presenter preferences under `[overlay]`.

```toml
[overlay]
font_size = 12.0
alignment = "left" # left, center, right
```

The native window uses the CLI's config reader/writer, so theme, registry and
other fields survive preference changes. Existing speaker-notes.json settings
migrate automatically on install or the next CLI/window launch. A pre-existing
[overlay] section wins; the old JSON is removed only after a successful migration.
Malformed legacy JSON is preserved with a warning for manual recovery.

F9 advances and F7 previews the previous completed step. Depending on your
keyboard settings, use Fn+F9 / Fn+F7. In the terminal, s starts preparation,
c confirms completion when offered, and q / Ctrl+C stops the session.
**Stop** on the panel also ends the session. Use `--no-overlay` for terminal controls.

Back and forward through completed steps show read-only previews. During
preparation and waits, **Stop** is the available scenario action. **Confirm
completion** accepts a verified manual step, including a step the adapter explicitly reported as
failed. A timeout or invalid reply leaves the outcome unknown and locks the
session for inspection and stopping. The panel displays the adapter's progress
and error detail.

The bundled Chrome rehearsals verify the expected view before each action.
**Off route** and **Info** turn red on a detour. Follow the short instruction
in **Info**, then select **Check route**. Back in Wikipedia search, the
check repeats the search if its result is missing. A successful check brings
back the transcript and enables **Forward** for the pending step.
The terminal shortcut is r. With terminal-only controls, the terminal may
retain keyboard focus while the expected Chrome tab is visible.

### Dry run

`demio run ID --dry-run` (or d in the selector) starts from the same introduction.
After **Start scenario**, F9/F7 move through the cues. A supporting adapter
may open or focus read-only rehearsal views. Outside a terminal the command
prints the introduction and script. `--auto` executes a live run directly and
requires a scenario without manual confirmation steps.

The presenter and both popovers request legacy capture exclusion
(`NSWindow.sharingType = .none`).
For a recording with the presenter visible, select the demo application window
or use a display recorder that explicitly excludes Demio. With region capture,
place the panel outside the selected area. Check a short saved recording on the
recording Mac before the demo. Full-display recorders can include the panel even
with the legacy flag set. [ScreenCaptureKit capture filters](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos)

## Rehearsals

Use `demio init` in a fresh directory to generate the reference rehearsals.
Install Python 3.10 or later and Google Chrome in `/Applications`. Each rehearsal uses its own profile,
fixed viewport, English locale and light appearance. Watch the mouse, typed
search, real link clicks and wheel scrolling on Wikipedia, Wikidata and IANA.
Try **Wikipedia and Wikidata** for a complete automatic walkthrough or
**Wikipedia manual recovery** for the Work click and **Confirm** gate.
**IANA download, timer and confirmation** verifies a published archive.

```sh
demio init ~/.toolbelt/demio/demos/my-demo
cd ~/.toolbelt/demio/demos/my-demo
python3 -m venv .venv
.venv/bin/python -m pip install playwright==1.63.0
demio register demo.json
demio list
demio
```

The generated README describes each route, the intentional timeout and error,
and the Mac file walkthrough. The latter uses the desktop by default and asks
the presenter to confirm that both lines are visible in TextEdit. To choose
a different destination, set `parameters.directory` in
`scenarios/mac-file/scenario.json` to an existing writable folder.
Existing generated demos keep their own files. To use the current examples on
another Mac, scaffold and register a fresh folder after installing this bundle.

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

## Sharing a demo

Use the [portable delivery contract](docs/portability.md) before handing a demo to another machine. Copying manifests/adapters and passing doctor on the sender does not establish a reproducible destination setup.
