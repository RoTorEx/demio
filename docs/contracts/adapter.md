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
Scenario schema 1 retains the step/run contract and supports optional demo,
parameters and type (`browser` or `system`). Type is display metadata.
Legacy standalone scenarios still define their own adapter and
workspace. Unknown fields are rejected in both manifests.

Scenario fields override inherited demo fields. Inherited workspace resolves
beside demo.json; an explicit scenario workspace resolves beside that scenario's
file. Commands execute in the resulting workspace. Parameters merge shallowly:
scenario keys replace the same demo keys. Demio passes the effective JSON object
in DEMIO_PARAMETERS and the canonical scenario path in DEMIO_SCENARIO. Adapters
own interpreting routes, inputs and other parameter values.

Use JSON arrays for ordered inputs such as the tabs to open. Demio may serialize
object keys in a different order from the manifest. Record the window and tab
handles returned by the browser so later steps and cleanup address the same
session-owned tabs.

Run `demio validate demo.json`, register the demo, doctor each scenario ID, then
rehearse the actual interactive paths. Batch registration checks all members and
ID conflicts before changing the registry. Scenario IDs and original creation
timestamps are stable. Step IDs must match adapter operations. Shared code
belongs in shared/ or the demo's established common location; do not duplicate
adapters merely for narration or route differences.

Each scenario can supply an intro cue for step zero, separate from the first
action. Selecting a scenario displays the intro. Start scenario launches the
adapter and calls prepare; a successful reply reveals step one.

Each step supplies id, title, role, say, and optional wait (a tip shown in red),
confirm (true to require presenter confirmation after a successful run), and
timeout_seconds (120 by default). Session seconds default to 7200. Narration is spoken by the presenter;
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
| verify | Optional read-only check of the expected view for the upcoming step |
| run | Execute the named uncompleted step and verify its visible result |
| preview | Focus/show the named completed step without business writes |
| rehearse | Dry run only: open or focus windows, without step once and then per step; never write or change state |
| cleanup | Close owned browser contexts and services |

Keep stdout strictly JSON lines. Send logs to stderr. Replies must use the exact
active numeric request ID. Each reply may contain up to one MiB of JSON.
The presenter displays the latest progress detail for the active request and
consumes each completion. The two event kinds are shown below.

```json
{"id":1,"event":"status","detail":"Waiting for approval status"}
{"id":1,"event":"done","ok":true}
```

Set optional `"waiting": true` on a status event to display Waiting with this
detail. An adapter can use this while polling an application result or waiting
for a timer. Send done after verifying the expected result for this request.

Failure example:

```json
{"id":1,"event":"done","ok":false,"error":"Approval did not become visible"}
```

A matching `done` with `ok: true` completes an automatic step and unlocks
presenter confirmation for a gated step. An explicit `ok: false` on a run step
allows the presenter to confirm a verified manual completion. Out-of-order IDs,
malformed JSON, EOF and timeouts leave the outcome unknown. The presenter can
inspect and stop the session before starting a fresh run.

A successful prepare reply can opt into interactive route verification by
returning `"verify_route": true`. Verify receives the upcoming step ID and checks
the live view without changing focus, opening tabs or issuing input. Demio
checks before enabling Forward, each half second while ready, and before
dispatch. A rejected check keeps the same step pending. After restoring the
view the presenter explicitly chooses Check route. A run reply can include
`"ok": false, "route_changed": true` for a detour before a business mutation.
Demio treats that result as a route check rather than manual step completion.
Adapters receive `DEMIO_TERMINAL_CONTROLS=1` in terminal-only and automatic
runs, where the terminal may hold keyboard focus.

Keep browsers and application state in the persistent process. Store disposable
profiles in the session directory. Do not detach or daemonize children. Stop
gives the adapter up to two seconds to reply to cleanup before Demio terminates
its process group. The session limit bounds this grace period when the action's
outcome is pending or unknown. The adapter owns release of external resources
and budgets. `--auto` deliberately
executes all automatic steps; any confirmation requirement refuses the scripted
run before preparation. `--dry-run` sends only `rehearse` and `cleanup` after
explicit Start, and an
adapter that refuses `rehearse` leaves the dry run as narration only.

## Native windows

The macOS helper reads `panel.json` from the owned session directory. It
displays the CLI's available actions and returns a command with the visible
frame's generation. The CLI discards commands from old frames. Stop works
in every phase. The presenter combines controls, status and notes in one
window that retains its position and size for the session. F7/F9 handle Back and
Forward. Hover over the information button to see shortcuts and the wait legend.
The panel observes changes to the session directory for new frames and closes
when the owning CLI exits.
The native helper also records the current foreground application PID in the
temporary session directory when macOS reports an activation. The Chrome
adapter compares it with the Chrome process it owns.

## Transfer to another machine

Follow [portable demo delivery](../portability.md). Check and preparation must distinguish a missing dependency from an application failure and explain the supported recovery. Do not bind browser-only scenarios to an unrelated sender checkout, profile file or mandatory personal tool. Preserve applicable existing stop guards. Real retained-run presentations require a supported data/runtime bootstrap; source instructions alone cannot recreate that state.
