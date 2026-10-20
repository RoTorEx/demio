# Project adapter contract · schema 1

A project agent should prepare its demo environment, own the workflow in the
project, and register `demo.json` with its declared scenarios. Follow that project's authorization rules;
registration does not grant permission for external writes or paid calls.

Start from `demio init DIRECTORY`. A demo is the entire created folder; its
`demo.json` lists scenarios and owns shared defaults. Each
`scenarios/<variant>/scenario.json` references `../../demo.json` and owns its ID,
name, description, creation time, ordered steps and parameter overrides.

Read the generated AGENTS.md, explain this terminology and the controls to the
user, propose a brief and agree audience, outcome, variants, target environment
and allowed effects before implementing product behavior. Retain accepted choices
in the demo README. Reuse existing authorization and keep independent preparation
moving while a material choice is unresolved.

Demo format (schema 1): id, name, optional project/description, workspace (default
`.`), adapter argument array, optional session_seconds, parameters object, and
scenarios (an explicit nonempty list of relative paths inside this demo folder).
Scenario schema 1 retains the existing step/run contract and adds optional demo
and parameters. Legacy standalone scenarios still define their own adapter and
workspace. Unknown fields are rejected in both manifests.

Scenario fields override inherited demo fields. Inherited workspace resolves
beside demo.json; an explicit scenario workspace resolves beside that scenario's
file. Commands execute in the resulting workspace. Parameters merge shallowly:
scenario keys replace the same demo keys. Demio passes the effective JSON object
in DEMIO_PARAMETERS and the canonical scenario path in DEMIO_SCENARIO. Adapters
own interpreting routes, inputs and other parameter values.

Run `demio validate demo.json`, register the demo, doctor each scenario ID, then
rehearse the actual interactive paths. Batch registration checks all members and
ID conflicts before changing the registry. Scenario IDs and original creation
timestamps are stable. Step IDs must match adapter operations. Shared code
belongs in shared/ or the demo's established common location; do not duplicate
adapters merely for narration or route differences.

Each step supplies id, title, role, say, and optional timeout_seconds (120 by
default). Session seconds default to 7200. Narration is spoken by the presenter;
role labels do not switch application accounts. Neither registration nor doctor
proves that product actions are safe or successful. Verify visible results and
appropriate failure boundaries. Never infer credentials, write permission or a
paid model budget.

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
