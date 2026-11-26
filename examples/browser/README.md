# Live browser rehearsals

Use the [generated rehearsals](../../README.md#rehearsals) to browse
Grace Hopper and Katherine Johnson, follow Ada Lovelace into Wikidata, and
wait for an IANA download. Each scenario JSON owns its cue and ordered
steps. `web.py` owns the shared Chrome actions and visible checks.

The standalone `channel.json` uses the stable ID `browser-demo`. It starts
at Wikipedia search, types Alan Turing, clicks the matching article,
scrolls down and closes the rehearsal window. Install Python 3.10 or later
and Google Chrome in `/Applications`. Run the following commands here.

```sh
python3 -m venv .venv
.venv/bin/python -m pip install playwright==1.63.0
demio validate channel.json
demio register channel.json
demio doctor browser-demo
demio run browser-demo
```

Step zero is the introduction. Press **Start** to prepare Chrome and
**Forward** to perform the current action. **Stop** ends the session.

The `article` route uses `parameters.query` as the search text and the
English Wikipedia article title.
