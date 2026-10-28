# Editing scenarios

A **demo** owns an entire folder and its reusable components. A **scenario** is
one route through that demo. A **step** is one ordered action/view within a
scenario. Registering the demo stores its scenario paths; it does not copy files.

```text
identity-graph/
  AGENTS.md
  README.md
  demo.json
  shared/
    adapter.py
    assets/
  scenarios/
    investigation/scenario.json
    role-change/scenario.json
    onboarding/scenario.json
```

The demo owns the project identity, environment, shared adapter and default
parameters. Scenarios own their names, descriptions, timestamps, step order,
narration and parameter overrides. Existing demos can keep shared modules in an
established root location; shared/ is the new scaffold convention.

Press e in the selector to open the owning demo root and selected JSON in VS
Code. Press f to reveal that JSON in Finder. A missing file's surviving demo
folder remains openable in Finder. These actions never execute the adapter.

## Creating a demo with an agent

`demio init ~/.toolbelt/demio/demos/my-demo` creates the whole structure, including two local browser
variants, one shared adapter, AGENTS.md and README.md. Give the agent this demo
folder. AGENTS.md requires it to explain terminology and controls, discuss the
audience, intended result, variants, environment, data, allowed writes/budgets,
language and recording needs, then propose a concrete brief and agree unresolved
choices before implementing behavior. The README retains the agreed brief.
Existing decisions stay in effect; routine implementation choices do not need
repeated approval. See [the authoring instructions](../templates/demo/AGENTS.md).

## Demo defaults and scenario differences

A minimal demo.json:

```json
{
  "schema_version": 1,
  "id": "ABC123",
  "name": "Identity Graph",
  "project": "Example",
  "description": "Investigate access using reusable browser actions.",
  "workspace": ".",
  "adapter": [".venv/bin/python", "-u", "shared/adapter.py"],
  "parameters": {"route": "/graph", "query": "Default question"},
  "scenarios": ["scenarios/investigation/scenario.json"]
}
```

A scenario references its owner and changes only the necessary defaults:

```json
{
  "schema_version": 1,
  "demo": "../../demo.json",
  "id": "XYZ789",
  "name": "Role change",
  "description": "Inspect retained access after a role change.",
  "parameters": {"query": "Find retained access"},
  "steps": [
    {"id": "show", "title": "Inspect access", "role": "Operator", "say": "I inspect the retained access."}
  ]
}
```

The adapter must implement show and use the route/query parameters. Effective
parameters are JSON in DEMIO_PARAMETERS, and DEMIO_SCENARIO is the selected source
path. Merging is by top-level key; nested values replace rather than deep-merge.
Demo workspace defaults resolve at the demo root. Explicit scenario workspace
overrides resolve beside that scenario file. Commands execute in that workspace.

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
| `workspace` | Optional override relative to this scenario; otherwise inherit the demo workspace. |
| `adapter` | Optional scenario override of the shared command and arguments; executable paths resolve in the effective workspace. |
| `demo` | Relative reference to the owning demo.json. Keep it correct when moving files. |
| `parameters` | Scenario-only route/input differences; override demo parameter keys. |
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
demio validate /path/to/demo/demo.json
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

## Storage ownership

Keep personal demos under ~/.toolbelt/demio/demos/<demo-name>. Global settings
are in ~/.toolbelt/demio/config.toml; demo settings stay in demo.json inside the
demo folder. Product upgrades replace bin only. Keep application checkout paths
in demo parameters rather than deriving them from this folder's parent count,
so moving a demo preserves its target. Do not put private demo data in the public
product repository. External project runtime data remains owned by that project.
