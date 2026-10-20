# Demio product truth

## Purpose and actors

A presenter needs to narrate a repeatable demo while automation performs visible
application steps. A project agent prepares the environment, manifest, and
adapter. The presenter registers the channel, selects its ID, speaks, and presses
Next. Viewers see the application; the presenter reads notes in the same window
as the controls.

## Concepts and invariants

A channel has a stable ID, readable name, project, description, and creation
time. Names display with `Project · ` as a prefix, added once; JSON also preserves
the raw name. Description states the visible flow and expected result. An agent
can derive naming and descriptions from a natural-language brief. Demio itself
does not generate workflows with AI.

`init` records creation time at scenario creation in RFC3339. `register` fills a
missing timestamp in manually authored manifests and preserves existing values.
Re-registering edits does not reset the date. Manifests retain source offsets;
all human-readable timestamps use Europe/Warsaw with date-specific CET/CEST and
UTC offset. Lists and the selector sort by actual instant, newest first, then ID
for ties; unknown dates come last. Legacy manifests derive a missing project from
the workspace directory and may have empty descriptions. Their unrecorded dates
fall back to filesystem creation time, which is an approximation for copied or
checked-out files. Unavailable manifests use the last registered metadata snapshot.
Metadata tests cover date order, edits, missing manifests, and summer/winter time.

The interactive selector follows GTD's two-pane TUI: a scrollable list contains
Warsaw creation date and name, clipped to fit; IDs appear only in details. The
detail pane groups information by scope and owner: scenario/Presenter (name and
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
The selector opens the selected manifest and containing folder in VS Code with
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
cue, including newly loaded steps. Font size and alignment persist beside the
CLI config in `speaker-notes.json`; malformed settings fall back to defaults and
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

- `src/registry.rs`: manifests, stable identity, configuration.
- `src/selector.rs`: two-pane terminal demo browser and details.
- `src/run.rs`: navigation, journaling, terminal and native integration.
- `src/protocol.rs`: persistent adapter, timeouts, process lifecycle.
- `native/Overlay.swift`: controls, movable/resizable speaker notes.
- `docs/scenarios.md`: scenario structure and manual editing workflow.
- `docs/contracts/adapter.md`: project-agent integration and protocol.
- `examples/browser`: local browser walkthrough.
- `scripts`, `.github/workflows`: packaging, installation, releases.
