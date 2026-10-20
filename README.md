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

`list` shows stable IDs, project-prefixed names, descriptions, and creation times
in Europe/Warsaw with the date's CET/CEST offset. Newest scenarios appear first;
the selector uses the same order.
`run` takes an exact ID, independent of list order. JSON output adds `project`,
`description`, `created_at`, and `display_name` while retaining the original `name`.
Run `demio` without arguments for the two-pane TUI, following GTD: the list
shows Warsaw creation date and as much of the name as fits; details show the full
name, description, and ID, followed by creation time, manifest, workspace,
session limit, and steps. Details are grouped by scope and owner: Presenter,
Registry, Project adapter, and Demio. Colored headings and labels separate these
groups; date and time use separate fields. List columns have a clear gap and
keep the year, and names use ` | ` as their display separator. Theme colors
follow GTD roles for borders, labels, dates, names, and selection. Long list cells are
clipped; the full ID and name remain in details. Use ↑/↓ or j/k to select,
PgUp/PgDn to scroll details, Enter to run, and q/Esc to quit. The list stays on
the left and details on the right at every terminal width. Empty registries
show registration guidance; unavailable demos show the error and cannot launch.
Outside a terminal, `demio` still prints the plain list. Registration stores a
reference to the manifest, so project-owned workflow changes are read on launch.
`demio init` records `created_at` automatically in the manifest. For manually
authored manifests, `register` adds it if missing and preserves it on subsequent
registrations. Registration therefore writes a missing timestamp to the project
manifest. Existing legacy manifests remain readable: the project defaults to the
workspace directory name, description may be empty, and unrecorded dates use
filesystem creation time where available (otherwise displayed as unknown).
Metadata snapshots keep unavailable channels identifiable.

Use `register FILE --replace` to explicitly replace an ID registered elsewhere.
`unregister ID` removes only the registry entry.

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
