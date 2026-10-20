# Demo brief and editing guide

This folder owns one demo. demo.json defines its shared identity, environment,
adapter, parameters and scenario list. Each scenarios/<variant>/scenario.json
owns a route, ordered steps, narration and parameter overrides. Both example
scenarios reuse shared/adapter.py and shared/index.html.

## Brief to agree with the user

- Audience and purpose: to be agreed.
- Visible outcome: a local request is submitted, approved and verified.
- Scenario variants: standard request and expedited request.
- Shared components: local browser setup, request/approval actions and checks.
- Per-scenario differences: input text and presenter narration.
- Target: local browser example; no external writes or paid calls.
- Presenter language and target duration: to be agreed.
- Recording requirements: to be agreed; Demio does not record video.

The scaffold is a local example, not evidence of an approved product workflow.
For a new real demo, follow AGENTS.md and agree the brief before implementation.

## Structure

```text
demo.json
AGENTS.md
README.md
shared/
  adapter.py
  index.html
scenarios/
  request/scenario.json
  expedited/scenario.json
```

Metadata and narration belong in scenario JSON; route/input differences belong
in parameters. Clicks, account switching and visible-result checks belong in the
shared adapter. Adding a step requires matching adapter behavior for its ID.
Do not change existing scenario IDs or original creation timestamps during edits.

## Prepare and launch

From this demo folder:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install playwright==1.63.0
PLAYWRIGHT_SKIP_BROWSER_GC=1 .venv/bin/python -m playwright install chromium
demio validate demo.json
demio register demo.json
demio list
demio
```

Run doctor for each scenario ID printed by list before rehearsing it. Registration
stores source paths; later edits are loaded on the next run. Reopen the selector
to refresh its metadata. e opens this demo root and the selected scenario in VS
Code. f reveals the scenario in Finder. The terminal lists navigation controls.
