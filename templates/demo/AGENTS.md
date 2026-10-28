# Demo authoring instructions

Read this file, `README.md`, `demo.json`, and the relevant scenario before work.
Work personally; never spawn subagents. Preserve user data and unrelated work.

## Terminology and ownership

- A **demo** is this complete folder: one purpose, demo.json, instructions, shared
  environment, adapters, fixtures, and one or more scenarios.
- A **scenario** is one route through the demo: its own stable ID, ordered steps,
  narration, role labels, description, parameters and creation timestamp. It
  references demo.json; it is not another copy of the application or adapter.
- A **step** is one named action or view. Its ID must be implemented by the adapter.
- **Shared components** implement reusable setup, navigation, actions, verification
  and cleanup. Keep them in shared/ (or this demo's established shared location).
- **Parameters** hold scenario differences such as routes, fixture selectors or
  input text. Reuse components rather than copying an adapter for each variation.
- **Demio** owns selection, sequencing, timeouts, controls, narration display,
  protocol transport and process cleanup. It does not infer product actions,
  speak, record video, or provide permission/credentials for external changes.
- The **project adapter** owns application behavior, accounts, browser state,
  safe preparation, result verification, read-only previews and owned resources.
- The **presenter** speaks the notes and advances at their own pace.

## Agreement before a new demo or scenario

When the user asks to make a new demo, explain this model and the proposed
folder/scenario structure in plain language before writing executable behavior.
Discuss the intended audience, purpose, visible outcome, duration, application
and target environment, scenario variants, roles, available data, recording
needs, setup, and which actions may change data or spend money.

Propose a concrete brief: demo name, scenario names, routes, ordered steps,
expected visible checks, shared components, scenario-only parameters, narration
language, and any unanswered decision. Explain how to launch the selector and
use Next/Back before asking the user to agree to the brief. Wait for agreement
before implementing behavior dependent on that decision. Do independent reading
and preparation while waiting. Reuse decisions already given; do not ask again
for routine choices within approved scope. User clarifications update the brief.

Keep the agreed brief in this demo's README.md. Write project artifacts in
English unless the user explicitly requests another language for that artifact.
Never invent credentials, authorization for writes, a paid budget, or facts about
an environment. A working doctor command is not proof that a demo succeeded.

## Implementation and verification

1. Define demo-wide defaults in demo.json and list each scenario file explicitly.
2. Put variants in scenarios/<variant>/scenario.json. Keep IDs and original
   created_at timestamps stable. Relative scenario demo paths resolve beside the
   scenario; inherited workspace paths resolve beside demo.json. Explicit
   scenario workspace overrides resolve beside that scenario's file.
3. Share adapter components. Read DEMIO_PARAMETERS (a JSON object) for effective
   merged defaults/overrides, and DEMIO_SCENARIO for the selected manifest path.
4. Implement check, prepare, run, preview and cleanup. Keep stdout JSON-only and
   logs on stderr. Report success only after the expected visible result is
   verified. Back/preview must not repeat writes. Timeouts have no automatic retry.
5. Run demio validate demo.json, register the demo, and doctor each scenario.
   Rehearse the real interactive flow and inspect failure/exit cleanup. Use
   --auto only when the approved scope includes executing every action.
6. Explain to the user what was built and tested, any limits, where to edit
   narration/parameters versus shared actions, and the exact launch command.

## Presenter controls

Run `demio`. Scenarios are grouped under their owning demo in the left pane;
the right pane describes demo-wide components and the selected scenario.
Arrow keys/j/k select; Enter starts the exact scenario. PgUp/PgDn scroll details
(on a MacBook, Fn+Up/Fn+Down). e opens the demo folder and selected JSON in VS Code;
f reveals the scenario in Finder. q/Esc leaves the selector.

During a demo, one floating window contains Back, Next and narration. The
presenter reads aloud. F9/Enter advances; F7 previews a completed step. Back is
viewing, not undo. q/Ctrl+C in the terminal ends the session. Font size and
alignment persist. Capture exclusion depends on the recorder: inspect a short
saved recording on the actual Mac before promising that the notes are hidden.

## Storage ownership

Keep personal demos under ~/.toolbelt/demio/demos/<demo-name>. Global settings
are in ~/.toolbelt/demio/config.toml; demo settings stay in demo.json inside the
demo folder. Product upgrades replace bin only. Keep application checkout paths
in demo parameters rather than deriving them from this folder's parent count,
so moving a demo preserves its target. Do not put private demo data in the public
product repository. External project runtime data remains owned by that project.
