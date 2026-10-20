# Project adapter contract · schema 1

A project agent should prepare its demo environment, own the workflow in the
project, and register `channel.json`. Follow that project's authorization rules;
registration does not grant permission for external writes or paid calls.

Start from `demio init DIRECTORY`. Adapt the JSON manifest's ID, name, workspace,
argument-array adapter command, session_seconds, and steps. Set `project` to the
project's readable name and `description` to what viewers will see and the
expected outcome. Keep `name` concise without repeating the project: Demio
renders `Project · Name`. The agent may derive these fields from the user's
natural-language request; Demio does not call an AI model to generate them.
`init` sets `created_at` automatically; preserve it when editing or regenerating
the scenario. If authoring JSON manually, omit that field and let `register`
record the timestamp, rather than inventing one. Dates are RFC3339 with an offset,
preserved in the manifest and rendered in Europe/Warsaw. All listings show newest
creation times first, with ID ordering as a deterministic tie-breaker. Each step supplies a
unique ID, title, role, `say` text for the presenter, and optional timeout_seconds
(default 120). The default workspace is the manifest's directory. Relative
workspaces and adapter executable paths resolve there. IDs are permanent,
non-secret identifiers; names may change. `schema_version` must be 1.

Use `demio validate FILE` before registration. Install project dependencies, then
register the manifest and run `demio doctor ID`. Test the real application and
appropriate failure states before handing the channel to the presenter. An agent
should supply narration identifying role changes, intentionally paced input,
smooth scrolling, target highlights, and checks that the intended state became
visible. The core does not impose browser timing or invent business actions.

## Persistent process

Demio starts the adapter once, in the workspace, with `DEMIO_WORKSPACE` and
`DEMIO_SESSION_DIR`. Standard input carries one JSON request per line:

```json
{"id":1,"op":"run","step":"submit","session_dir":"/runtime/session"}
```

Operations:

| Operation | Meaning |
|---|---|
| check | Read-only dependency/environment check for doctor |
| prepare | Prepare a fresh environment once before interaction |
| run | Execute the named uncompleted step and verify its visible result |
| preview | Focus/show the named completed step without business writes |
| cleanup | Close owned browser contexts and services |

Keep stdout strictly JSON lines. Send logs to stderr. Replies must use the exact
active numeric request ID and one of these events:

```json
{"id":1,"event":"status","detail":"Waiting for approval status"}
{"id":1,"event":"done","ok":true}
```

Failure example:

```json
{"id":1,"event":"done","ok":false,"error":"Approval did not become visible"}
```

Only `done` with `ok: true` marks a step complete. Out-of-order IDs, malformed
JSON, EOF, errors, and timeouts fail the operation. There are no automatic retries
because a write may already have happened. Stop and inspect the project before
starting a fresh session. The adapter should not reply before verification.

Keep browsers and application state in the persistent process. Store disposable
profiles in the session directory. Do not detach/daemonize children. Demio sends
cleanup, then terminates the adapter's process group on exit. Releasing external
resources and budgets remains the adapter's responsibility. `--auto` deliberately
executes all steps; use only after the project authorizes those actions.

## Native windows

The macOS helper reads `panel.json` from the owned session directory and sends
`next` / `previous` commands to the Rust CLI. It never invokes an adapter itself.
The combined presenter window contains Back/Next and the active step's `say`
text and role, and retains user-adjusted position and size during the session.
The window closes when the CLI exits. F7/F9 work globally; no F1 hide/show control exists.
