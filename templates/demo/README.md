# Demo brief and editing guide

This folder owns one demo. demo.json defines its shared identity, environment,
adapter, parameters and scenario list. Each scenarios/<variant>/scenario.json
owns a route, introduction, ordered steps, narration and parameter overrides.

## Rehearse

| Scenario | What to check |
| --- | --- |
| Wikipedia · Grace Hopper | Type the Wikipedia address, search for the name, scroll through the article, and click the tab's close button. |
| Katherine Johnson · Follow Hidden Figures | Find the biography, follow its link to Hidden Figures, and wait for the linked article to load. |
| Ada Lovelace · Wikipedia and Wikidata | Open the article, scroll, follow its Wikidata link into a second tab, and return to Wikipedia. |
| IANA download, timer and confirmation | Click Release History and the published release. Wait through the reading timer, verify the downloaded archive and choose Confirm. Return to the history and close. |
| Mac file walkthrough | Type cd, touch, cat, echo, and cat in a fresh Terminal window with a one-second pause after each command. The greeting stays in the file. |

The public routes need internet access, Python 3.10 or later, and Google Chrome
in `/Applications`. Allow Accessibility for the app that launches Demio
in System Settings. Start opens a blank tab in an isolated profile. The first
Forward types the site address. The search uses Wikipedia's
own field and matching suggestion. Keep the rehearsal tabs in their original
order. If you switch apps or tabs or open a different page, **Back** shows
earlier cues. Restore the rehearsal view and choose **Check route** to
re-enable **Forward**. The Mac route
defaults to `~/Desktop`.
Set `parameters.directory` in `scenarios/mac-file/scenario.json` to choose an
existing writable folder. macOS may request Automation access for Terminal. The
finished document remains in the chosen folder.

For a product demo, follow AGENTS.md and retain its agreed brief here.

Metadata and narration belong in scenario JSON; route/input differences belong
in parameters. Clicks, account switching and visible-result checks belong in the
shared adapter. Adding a step requires matching adapter behavior for its ID.
Separate different transcript topics with blank lines. Use angle brackets for
short cues the presenter keeps private, such as `<Pause for the result>`.
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
Hover over **ⓘ** for shortcuts. Use **Stop** to end the rehearsal.
