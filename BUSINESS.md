# Demio product truth

## Product components

Demio is a product under personal/projects/dev-tools/demio, comprising demio-cli and
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
run ID, readable name, description, optional type, creation time, ordered steps,
narration and parameter overrides. A step belongs to a scenario and invokes
adapter behavior by its step ID. The scenario type is browser or system metadata.
A legacy manifest without a type displays Unspecified.
Optional development token metadata records input, cached input, output and
the accounting basis. The cached count is part of input. The selector's
Development tokens section displays the total of input and output, the
allocation basis and the cache breakdown with compact units.

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

The scaffold creates browser and native Mac rehearsals with local authoring
instructions. The browser cases use public Wikipedia, Wikidata and IANA pages
in an isolated Google Chrome profile. The audience sees the pointer, paced
typing, real link clicks and smooth wheel scrolling. Search results and subsequent
views follow visible input. Manual recovery requires the presenter to select
Work and confirm before the adapter verifies and scrolls that section. The tab
rehearsal follows the published Wikidata link into an owned second tab. Closing
a rehearsal closes its window, and Back after Close previews the cue without
reopening it. The Mac rehearsal creates one uniquely
named document in the selected folder, which defaults to the desktop. A
completed document remains available in TextEdit. Generated demo/scenario IDs use six
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

The selector shows a tree of demos and scenarios on the left and details on the
right. Group labels start at the left and wrap at the pane edge. Each scenario
shows ID, Warsaw creation date as DD.MM.YY, and name in evenly spaced columns.
The ID column shows six characters and an ellipsis for longer IDs. The full
stable ID appears in Metadata and remains the CLI identity. Scenario names
end in an ellipsis when they exceed the available space.
Metadata starts with scenario identity and creation time. Description and
Steps follow, with Introduction as step zero. Development tokens, the maximum
run time after setup and the source paths have separate sections below.
The mouse wheel scrolls the pane under the pointer and keeps the
selected scenario. PageUp/PageDown scroll details. Unavailable scenarios show
the error in Metadata and refuse to open. An empty registry shows registration
guidance.
Theme roles distinguish the two panes, headings, dates and the current selection.
Calm uses dark text on its blue selection.
Selection previews the manifest. Enter opens its presenter at step zero.
Non-terminal output stays scriptable.
The selector opens the selected manifest and owning demo root in VS Code with
e, or reveals the file in Finder with f. These inspection actions never invoke
adapters. Invalid manifests remain openable; Finder can open a surviving parent
folder if the manifest is missing. Opening failures stay visible in the selector.

Ordering the channel list never changes identity. Manifests remain owned by their projects. Registry replacement
requires an explicit flag when it changes the source file. Removing registration
preserves source data.

A scenario's introduction is the step-zero cue. Selecting it exposes that cue
and the controls. Start scenario invokes the adapter and prepares the
environment. Successful preparation shows the first executable step with its
own narration. Next runs that step. A step contains its ID, title, role,
narration text, operation timeout, and optional confirmation requirement.
The presenter reads the text marked **Read aloud** below **Info**. Both sections
use regular-weight body text and distinct labels. The movable, resizable
presenter separates notes from navigation with a horizontal rule. Back, Start
and Forward stay on the first row, with a fixed-size state control next to Stop
on the second row. All controls stay visible. The gear sits at the right of
the scenario title beside the circled information button. The gear opens
settings by click and the information button opens the F7/F9 shortcuts and
wait legend on hover.
The panel initially sits at the lower-right so the left-side contents in the
browser rehearsals stay reachable. A−/A+ changes the spoken text size by two
points per click within 9–48 points. Left, center, and
right alignment apply to narration. Step titles, progress, and wait instructions
remain left-aligned in the fixed-size **Info** section.
The 11-point default is half the 0.1.10 default. The initial 360 × 240 point
content area keeps the panel compact during a demo. Presenters can resize the
panel or adjust the font, and longer notes wrap and scroll. Font size
and alignment persist in the shared config.toml [overlay] table (font_size and
alignment). The CLI owns TOML parsing,
atomic writes and legacy JSON migration; the native window calls its sibling CLI
through an internal settings interface, without interpreting TOML itself. Reads
and writes honor DEMIO_CONFIG and DEMIO_HOME. Preference writes preserve other
config values and comments. Normal theme/registry saves preserve the overlay table.
Valid legacy speaker-notes.json is imported once when [overlay] is absent, then
removed after saving; an existing [overlay] takes precedence. Malformed legacy
JSON is retained with a warning rather than discarded. Broken TOML blocks writes
and preserves source files. Missing/invalid individual preferences use defaults;
out-of-range sizes clamp to the range. Native smoke checks exercise both popovers,
button placement and availability, text styles, alignment, resizing, and preference reload.
Changing the range affects readability and control bounds.
Global F7/F9 are available while the helper is alive; there is no hide toggle.

Next executes the next uncompleted step once. Back and forward navigation
through completed steps invoke read-only preview. The adapter can report
progress and waiting detail. Next is blocked while the adapter is working or
waiting. Confirm completion accepts an explicitly gated step, or a reported
failure after the presenter has verified completion manually. The presenter then
advances with a separate Next. Timeout, lost transport and invalid replies lock
the session with unknown outcome for inspection and stopping. Each new run
creates a fresh environment. Adapters must make preparation safe for fresh runs.

An adapter can opt into read-only verification of the upcoming execution step.
The presenter checks before making Forward available, while ready, and before
dispatching the step. The 500 ms idle interval detects detours without
redrawing or writing a new panel frame on unchanged results. The public Chrome
adapter checks the expected tab, page and search prerequisites. A detected
detour disables Forward until the presenter restores the view and chooses
Check route. Checking a restored view leaves the pending step unexecuted.

A dry run lets the presenter learn the narration before a real run. After Start,
Demio asks the adapter to `rehearse` once (30 seconds, long enough for a browser to open) and
then per step; an adapter that refuses or times out leaves the dry run as
narration only. F9 and F7 move through cues. A printed rehearsal opens no adapter.
`--auto` deliberately starts the live adapter immediately and requires a scenario
without manual confirmation steps.

## Limits and ownership

The first minor supports macOS 14+ Apple Silicon, matching the owner's machine;
changing targets requires native build and install verification. The Rust core
uses a Swift/AppKit helper for native controls. Adapter languages are unrestricted.
No API keys, account defaults, or paid budgets are provided by the remote.
The bundled rehearsals incur no model spend. Browser demos read live public
pages and verify the downloaded IANA archive inside the session directory.

Defaults: 7,200 seconds per interactive session (two-hour rehearsal), 120 seconds
per operation (normal browser operations plus load margin). Manifest limits are
1–86,400 session seconds and 1–3,600 operation seconds, permitting long rehearsals
while rejecting zero/unbounded waits. Session time begins after preparation.
Session expiry locks navigation and terminates an in-flight adapter in both
interactive and scripted runs. Preparation uses the first step's timeout.
Stopping allows up to two seconds for the adapter's cleanup reply before Demio
terminates its process group. These are local operational
limits, never an authorization or money limit. Changing them affects interruption
and recovery; CLI timeout/failure tests protect the blocking boundary.

IDs accept 1–64 ASCII letters, digits, hyphens, underscores, starting alphanumeric,
so they remain predictable CLI arguments. Step IDs must be unique. JSON schema
version 1 rejects unknown manifest fields to catch agent mistakes. Validation and
registry tests protect these contracts.

## Boundaries

Adapters are trusted local executable code, not a sandbox. Their check, preview and
rehearse operations must be read-only. Preparation, browser scrolling/highlighting, writes,
verification, credentials, budget enforcement, and cleanup belong to the project
adapter. It must keep owned children in its process group and must not daemonize.
Demio terminates that group and deletes its per-session runtime directory at exit.
A parent crash cannot guarantee cleanup of arbitrary external side effects.

Presenter notes and controls should remain visible to the presenter and absent
from the demo recording. The native panel and both popovers request legacy
capture exclusion (`NSWindow.sharingType = .none`). The native smoke check verifies
this setting. Modern macOS ScreenCaptureKit recorders may ignore it.
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
- `examples/browser`: live Wikipedia, Wikidata and IANA rehearsals.
- `examples/mac`: Terminal and TextEdit file rehearsal.
- `scripts`, `.github/workflows`: packaging, installation, releases.

## Local product usage evidence

Record meaningful workflow outcomes locally, with bounded storage and no source
content. [Usage logging](docs/usage-logging.md) owns the event and retention
contract. Successful API calls, prepared artifacts and user-reported outcomes
must not be presented as broader business success.

## Portable demo delivery

When sharing a demo, the usable result includes declared dependencies, supported destination setup, non-secret fixture/runtime requirements, interactive account access and verification after relocation. Source-machine doctor success is not portability evidence. The [delivery contract](docs/portability.md) defines the boundary; project-specific data/bootstrap remain owned by the adapter/application, not inferred by the sequencing engine.
