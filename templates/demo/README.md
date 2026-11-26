# Demo brief and editing guide

This folder owns one demo. demo.json defines its shared identity, environment,
adapter, parameters and scenario list. Each scenarios/<variant>/scenario.json
owns a route, introduction, ordered steps, narration and parameter overrides.

## Rehearse

| Scenario | What to check |
| --- | --- |
| Wikipedia and Wikidata | Watch the cursor move to search, the query being typed and the result clicked. Scroll through Work, follow the real Wikidata link in tab two, return to Biography and close. |
| IANA download, timer and confirmation | Click Release History and the published release. Wait through the reading timer, verify the downloaded archive and choose Confirm. Return to the history and close. |
| Wikipedia late navigation and stop | Search for Ada Lovelace and leave Biography open until Outcome unknown. Click Work and observe that Forward is still blocked. Use Stop to close the rehearsal. |
| Wikipedia manual recovery | Watch the search and result click. When the expected-section check fails, click Work in Wikipedia's contents, verify the heading, and choose Confirm. Forward scrolls and then closes the rehearsal window. |
| Mac file walkthrough | Create a unique text file in Terminal, append a line, open the document in TextEdit and confirm that both lines are visible. |

The public routes need internet access, Python 3.10 or later, and Google Chrome
in `/Applications`.
Start opens a landing page in an isolated profile. The search uses Wikipedia’s
own field and matching suggestion. A ring makes the pointer visible in window
recordings. Keep the rehearsal tabs in their original order. The Mac route
defaults to `~/Desktop`.
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
demio validate demo.json
demio register demo.json
demio list
demio
```

Run doctor for each scenario ID printed by list before rehearsing it. Registration
stores source paths; later edits are loaded on the next run. Reopen the selector
to refresh its metadata. e opens this demo root and the selected scenario in VS
Code. f reveals the scenario in Finder. Select a scenario and read its step-zero
introduction. Click **Start** to prepare it, then **Forward** for step one.
Use **ⓘ** for shortcuts and **Stop** to end the rehearsal.
