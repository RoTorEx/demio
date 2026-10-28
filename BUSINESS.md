# Demio product truth

## Product components

Demio is a product under personal/products/demio, comprising demio-cli and
demio-overlay. demio-cli owns the terminal entry point (`demio`), TUI, registry,
adapter protocol, sequencing and process cleanup. demio-overlay owns the native
macOS presenter window and global controls, with no project-action authority.
The components are built from one Cargo workspace/product root, versioned and
packaged together, and installed side by side. The Rust package is demio-cli;
the stable public executable remains demio. Component README files explain
these boundaries.

## Installed home and storage

The source repository owns product code, templates and public examples. The
installed product home defaults to ~/.toolbelt/demio (DEMIO_HOME overrides it).
Its bin directory contains exactly one current CLI/overlay bundle; verified
updates replace it and preserve data. Old same-root version bundles are removed
only after successful replacement. A verified default-home installation also
retires recognizable executable bundles from the old ~/.x-cli-demio home, while
preserving user state there. Stop active runs before updating.
One top-level config.toml owns global settings, registry and [overlay] presenter
preferences. There is no separate speaker preferences file. Each demo lives
under demos/<name> with its own demo.json, scenarios, shared components and local
instructions. External registered demo paths remain supported without copying.
The installer imports missing legacy settings without overwriting new settings
or moving project source. Registrations are references and metadata snapshots,
not another demo configuration. Moving a demo requires explicit re-registration.

Session transport and owned browser files are temporary and deleted on exit;
there is no persistent resume state, sessions directory or routine backup store.
logs/demio.jsonl retains only timestamps, process/scenario identity, position and
status, excluding adapter payloads, parameters and narration. Rotation at 1 MiB
keeps one prior file to bound disk usage during repeated rehearsals. Installer
checks cover repeat replacement, migration from version symlinks, rejection of
corrupt archives and preservation of config/demo/log files. CLI checks exercise
isolated home paths and diagnostic records. Project adapters own their separate
application runtime state; this storage policy does not delete that source data.

## Purpose and actors

A presenter needs to narrate a repeatable demo while automation performs visible
application steps. A project agent prepares the environment, manifest, and
adapter. The presenter registers the demo, selects a scenario ID, speaks, and presses
Next. Viewers see the application; the presenter reads notes in the same window
as the controls.

## Concepts and invariants

A demo is one user-owned folder containing demo.json, AGENTS.md, README.md,
shared components and one or more scenarios. Demo metadata owns its name,
description, project, workspace, adapter defaults, shared parameters and an
explicit list of scenario paths. A scenario references its demo and owns a stable
run ID, readable name, description, creation time, ordered steps, narration and
parameter overrides. A step belongs to a scenario and invokes adapter behavior
by its step ID. Neither a scenario nor a step is another demo.

Demo-relative workspace defaults resolve at demo.json. An explicit scenario
workspace override resolves beside its scenario JSON. Adapter commands execute
in the effective workspace. Scenario fields override demo defaults; parameters
merge by top-level key. Effective parameters and the selected source path reach
the adapter as DEMIO_PARAMETERS (JSON) and DEMIO_SCENARIO. The adapter owns routes
and product behavior; Demio does not interpret parameter values.

Registering demo.json validates all declared scenarios and ID conflicts before
writing timestamps or saving the registry. Scenario files must be inside the
demo folder and refer back to it. Existing IDs and timestamps survive moving a
manifest with explicit --replace. Standalone legacy manifests remain readable.
Their metadata snapshots and demo membership remain visible when files become
unavailable. One missing sibling does not invalidate a runnable scenario.

The scaffold creates a demo, two parameterized scenarios, one shared adapter and
page, and local authoring instructions. Generated demo/scenario IDs use six
alphanumeric characters. The authoring agent must explain terminology and
controls, discuss audience/outcome/environment/allowed effects, propose a brief
and agree unresolved product choices with the user before implementing behavior.
Accepted decisions are retained in the demo README and reused without repeating
approval questions. Templates own this guidance, not historical chat messages.

`init` records creation time at scenario creation in RFC3339. `register` fills a
missing timestamp in manually authored manifests and preserves existing values.
Re-registering edits does not reset the date. Manifests retain source offsets;
all human-readable timestamps use Europe/Warsaw with date-specific CET/CEST and
UTC offset. Plain lists sort by actual instant, newest first, then ID
for ties; the TUI groups by owning demo, with newest scenarios first within
each group and groups ordered by their newest scenario; unknown dates come last. Legacy manifests derive a missing project from
the workspace directory and may have empty descriptions. Their unrecorded dates
fall back to filesystem creation time, which is an approximation for copied or
checked-out files. Unavailable manifests use the last registered metadata snapshot.
Metadata tests cover date order, edits, missing manifests, and summer/winter time.

The interactive selector follows GTD's two-pane TUI: demo headers group scenarios
in a scrollable list containing
Warsaw creation date and name, clipped to fit; IDs appear only in details. The
detail pane groups information by scope and owner: demo/shared components, scenario/Presenter (name and
description), identity/Registry (ID, project, separate creation date and time),
environment/Project adapter (manifest and workspace), execution/Demio (session
limit), and steps/Project adapter (step titles and roles). Blank lines separate
groups and path fields. Theme roles distinguish list/detail borders, headings,
labels, dates, names, errors, and selection, following GTD palettes. The list
retains the creation year and leaves three columns between date and name.
Displayed TUI names use ` | ` between name segments, without rewriting source
names or stored IDs. Selection updates
details without invoking adapters. Long cells are clipped, with full text in the
details pane. PageUp/PageDown scroll details; unavailable demos display their
error and cannot launch. An empty registry still opens the TUI with registration
guidance. The panels always sit side by side, list left and details right,
including narrow terminals. Existing stable IDs are preserved. Newest-first ordering and
exact run IDs remain unchanged. Non-terminal output stays scriptable.
The selector opens the selected manifest and owning demo root in VS Code with
e, or reveals the file in Finder with f. These inspection actions never invoke
adapters. Invalid manifests remain openable; Finder can open a surviving parent
folder if the manifest is missing. Opening failures stay visible in the selector.

Ordering the channel list never changes identity. Manifests remain owned by their projects. Registry replacement
requires an explicit flag when it changes the source file. Removing registration
preserves source data.

A step contains its ID, title, role, narration text, and operation timeout.
The presenter speaks; there is no synthesized audio. The presenter window
combines Back, Next, and speaker notes in one movable,
resizable panel. Controls and narration stay together when moving or resizing.
A−/A+ changes notes font size in two-point increments, from 14 to 48 points,
with a 22-point default. This reading range keeps small text legible and large
text usable with wrapping. Left, center, and right alignment apply to the full
cue, including newly loaded steps. Font size and alignment persist in the shared
config.toml [overlay] table (font_size and alignment). The CLI owns TOML parsing,
atomic writes and legacy JSON migration; the native window calls its sibling CLI
through an internal settings interface, without interpreting TOML itself. Reads
and writes honor DEMIO_CONFIG and DEMIO_HOME. Preference writes preserve other
config values and comments. Normal theme/registry saves preserve the overlay table.
Valid legacy speaker-notes.json is imported once when [overlay] is absent, then
removed after saving; an existing [overlay] takes precedence. Malformed legacy
JSON is retained with a warning rather than discarded. Broken TOML blocks writes
and preserves source files. Missing/invalid individual preferences use defaults;
out-of-range sizes clamp to the range. Native smoke checks exercise controls,
all alignments, resizing, and preference reload. Changing the range affects
readability and control bounds.
Global F7/F9 are available while the helper is alive; there is no hide toggle.

Next executes the next uncompleted step once. Back and forward navigation through
completed steps invoke read-only preview. They neither undo nor repeat writes.
An adapter error blocks further actions in that session. Timeouts leave outcomes
unknown, with no automatic retry. A new run is a new environment, not a resume.
Project adapters must make preparation safe for repeated fresh runs.

## Limits and ownership

The first minor supports macOS 14+ Apple Silicon, matching the owner's machine;
changing targets requires native build and install verification. The Rust core
uses a Swift/AppKit helper for native controls. Adapter languages are unrestricted.
No API keys, account defaults, or paid budgets are provided by the remote.
The bundled example is local and incurs no model spend.

Defaults: 7,200 seconds per interactive session (two-hour rehearsal), 120 seconds
per operation (normal browser operations plus load margin). Manifest limits are
1–86,400 session seconds and 1–3,600 operation seconds, permitting long rehearsals
while rejecting zero/unbounded waits. Session time begins after preparation.
An in-flight interactive operation is stopped when the session expires; scripted
`--auto` checks the session deadline between operations and uses operation limits
while waiting. Preparation uses the first step's timeout. Cleanup has a two-second
grace before the owned process group is terminated. These are local operational
limits, never an authorization or money limit. Changing them affects interruption
and recovery; CLI timeout/failure tests protect the blocking boundary.

IDs accept 1–64 ASCII letters, digits, hyphens, underscores, starting alphanumeric,
so they remain predictable CLI arguments. Step IDs must be unique. JSON schema
version 1 rejects unknown manifest fields to catch agent mistakes. Validation and
registry tests protect these contracts.

## Boundaries

Adapters are trusted local executable code, not a sandbox. Their check and preview
operations must be read-only. Preparation, browser scrolling/highlighting, writes,
verification, credentials, budget enforcement, and cleanup belong to the project
adapter. It must keep owned children in its process group and must not daemonize.
Demio terminates that group and deletes its per-session runtime directory at exit.
A parent crash cannot guarantee cleanup of arbitrary external side effects.

Presenter notes and controls should remain visible to the presenter and absent
from the demo recording. The combined native panel requests the same legacy capture
exclusion (`NSWindow.sharingType = .none`) as the original project-specific remote;
the native smoke check protects this setting on the single window. Modern macOS
ScreenCaptureKit recorders may ignore it, so the flag is not a universal guarantee.
Verify a short saved output with the actual recorder and macOS version. Where the
flag is ignored, exclude Demio in the recorder or keep the panel outside the
selected recording region. Screen recording itself, speech synthesis, a phone
endpoint, and generic AI-driven application control remain outside this first minor.

## Source map

- `demio-cli/src/demo.rs`: demo manifests, ownership, inheritance and parameters.
- `templates/demo`: generated user/agent guidance and brief.
- `demio-cli/src/registry.rs`: manifests, stable identity, configuration.
- `demio-cli/src/selector.rs`: two-pane terminal demo browser and details.
- `demio-cli/src/run.rs`: navigation, journaling, terminal and native integration.
- `demio-cli/src/protocol.rs`: persistent adapter, timeouts, process lifecycle.
- `demio-overlay/Overlay.swift`: controls, movable/resizable speaker notes.
- `docs/scenarios.md`: scenario structure and manual editing workflow.
- `docs/contracts/adapter.md`: project-agent integration and protocol.
- `examples/browser`: local browser walkthrough.
- `scripts`, `.github/workflows`: packaging, installation, releases.
