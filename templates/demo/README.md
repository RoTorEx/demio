# Demo brief and editing guide

This folder owns one demo. demo.json defines its shared identity, environment,
adapter, parameters and scenario list. Each scenarios/<variant>/scenario.json
owns a route, introduction, ordered steps, narration and parameter overrides.

## Rehearse

| Scenario | What to check |
| --- | --- |
| Wikipedia and Wikidata | Click Work in Wikipedia, open the related Wikidata entry in tab two, then select Biography in the original tab. Back revisits a completed view. |
| IANA download, timer and confirmation | Open the published release, wait for the reading timer, then verify the data archive downloaded from IANA. Confirm and use Next to return to release history. |
| Wikipedia late navigation and stop | Leave Biography open until Outcome unknown. Click Work on Wikipedia and observe that the presenter is still blocked. Use Stop. |
| Wikipedia manual recovery | Open Work in Wikipedia's contents when the expected-section check fails. Verify the heading, confirm and continue. |
| Mac file walkthrough | Create a unique text file in Terminal, append a line, and open the document in TextEdit. |

The public routes need internet access. The Mac route defaults to `~/Desktop`.
Set `parameters.directory` in `scenarios/mac-file/scenario.json` to choose an
existing writable folder. macOS may request Automation access for Terminal. The
finished document remains in the chosen folder.

For a product demo, follow AGENTS.md and retain its agreed brief here.

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
Code. f reveals the scenario in Finder. Select a scenario and read its step-zero
introduction. Click **Start scenario** to prepare it, then **Next** for step one.
Use **Stop** before Start to close an unused presenter.
