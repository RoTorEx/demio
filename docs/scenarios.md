# Editing scenarios

A scenario is a project-owned JSON manifest plus an executable adapter. Demio
registers the manifest's path; it does not copy the scenario into its own repo.
Several manifests can share one folder, adapter, and supporting modules.

In the selector, press **e** to open the selected manifest and its containing
folder in VS Code. Press **f** to reveal the manifest in Finder. If the manifest
is missing but its folder exists, Finder opens that folder. Invalid JSON can
still be opened for repair. These actions never call the adapter. Errors remain
in the selector's status line. The editor uses the `code` command, with the
macOS Visual Studio Code application as a fallback if that command is absent.

## Files and ownership

```text
demo/
  channel.json       # Metadata, narration, ordered steps, adapter command
  adapter.py         # Project actions and verification; Python is optional
  assets/            # Optional local files, fixtures, or browser page
```

`demio init ./my-demo` scaffolds a working local browser example with
`channel.json`, `adapter.py`, and `index.html`. Dependencies still need installation
as described in the main README. Projects can use different manifest names,
shared adapters, or additional support modules.

## What can be edited

| Field | Purpose and editing guidance |
| --- | --- |
| `name`, `project` | Readable scenario and project names. Editing these does not change its stable identity. |
| `description` | Explain the visible flow and expected result shown in the detail pane. |
| `steps[].title` | The step's human-readable heading. |
| `steps[].role` | The role displayed to the presenter; it does not switch application accounts itself. |
| `steps[].say` | Text the presenter reads aloud. It does not execute actions or synthesize speech. |
| `steps[].timeout_seconds` | Optional per-operation timeout: 1–3,600 seconds, default 120. |
| `session_seconds` | Session time limit: 1–86,400 seconds, default 7,200. |
| `workspace` | Working directory for the adapter; relative to the manifest's folder. Defaults to `.`. |
| `adapter` | Command and arguments as a JSON array. Project-owned executable paths must resolve in the workspace. |
| `steps` | Ordered sequence. New step IDs need matching adapter behavior; changes can affect dependencies between steps. |
| `id` | Stable registry/run identity. Preserve it for an existing scenario. For an independent copy, choose a unique ID and register it. |
| `steps[].id` | Unique action identifier understood by the adapter. Preserve it unless also updating the adapter. |
| `created_at` | Original creation timestamp; preserve it when editing. `init` sets it and registration fills it when absent. |
| `schema_version` | Keep `1`; unknown JSON fields are rejected. JSON does not support comments. |

A step object can look like this:

```json
{
  "id": "approve",
  "title": "Review and approve",
  "role": "Approver",
  "say": "I review the request and approve it. The status changes to Approved.",
  "timeout_seconds": 120
}
```

Changing narration, role labels, and display metadata only requires JSON edits.
Changing clicks, URLs, selectors, browser timing, role switching, or verification
requires adapter edits. Adding a JSON step alone does not implement its action.
Removing or reordering a step can break the state needed by later steps.

## Adapter responsibilities

The adapter is a long-lived process. Demio sends JSON-line requests for `check`,
`prepare`, `run`, `preview`, and `cleanup`. The adapter owns application actions,
account switching, dependency checks, and visible-result verification. The
`step` value matches `steps[].id`. Only a successful `done` response completes a
step. Keep stdout for protocol JSON; send diagnostics to stderr.

`preview` must remain read-only: Back previews a completed step without undoing
or repeating its writes. Timeouts never trigger automatic retries. A fresh run
starts a fresh environment; setup and cleanup belong to the adapter.

See [the adapter contract](contracts/adapter.md) for request and reply examples.

## Check and use changes

```sh
demio validate /path/to/demo/channel.json
demio doctor your-scenario-id
demio run your-scenario-id
```

`validate` checks the manifest and workspace without executing adapter code.
`doctor` invokes the adapter's read-only dependency/environment check. A manual
rehearsal checks actual application behavior. Avoid `--auto` unless running all
steps and their effects is intended.

Changes to the same manifest are loaded on the next run; registration need not
be repeated. Quit and reopen the selector to refresh its metadata and details.
Use `demio register FILE` for a new scenario or after moving the manifest; use
`--replace` explicitly if an existing ID moves to a different path.
