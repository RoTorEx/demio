# Changelog

## [Unreleased]

- Add bounded, metadata-only local product usage evidence; preserve business and recovery state.

- Preserve the active executable bundle when retired home paths are symlink aliases of the current Toolbelt installation.

## [0.1.9] - 2026-10-28

- Toolbelt migration: use `~/.toolbelt/<name>` for custom installations and update project-path references where applicable.

## [0.1.8] - 2026-10-23

- Retire old default-home executable bundles after a verified installation, preserve user state and keep one prepared local archive.

## [0.1.7] - 2026-10-20

- Move presenter font size and alignment into the shared config.toml [overlay]
  section. Preserve existing preferences through automatic legacy JSON migration,
  remove the migrated file, and retain other configuration during native edits.

## [0.1.6] - 2026-10-20

- Use ~/.x-product-demio for the installed product: top-level configuration,
  user demo folders, logs and one current CLI/overlay bundle. Verified upgrades
  replace binaries and preserve data, including migration of same-root legacy
  version installations and import of missing old preferences.
- Replace persistent last-session state with bounded diagnostic logs and use
  OS temporary storage for session files; document storage ownership and migration.

## [0.1.5] - 2026-10-20

- Organize Demio as a product under personal/products/demio, with separate
  demio-cli and demio-overlay components in one build/release workspace. Keep
  the public demio command stable and update editor project registrations.

- Separate demo containers from scenarios: shared defaults and parameterized
  adapters, per-scenario paths/narration, grouped TUI, batch registration, and
  source opening at the owning demo root.
- Scaffold two scenarios with shared components and local AGENTS.md/README.md
  instructions for terminology, ownership, controls, agreement and verification.

- Open selected scenarios in VS Code (e) or Finder (f), with visible launch
  errors and no adapter execution. Document scenario structure and manual edits.

- Add GTD-style color roles, a clear date/name column gap, and ` | ` name
  separators. Group details by scope and owner, with separate creation date/time
  fields and spacing around project-owned paths.

- Combine navigation buttons and speaker notes in one movable, resizable native
  presenter window, preserving font, alignment, hotkeys, and capture exclusion.
- Replace the selector with a GTD-style two-pane TUI: scrollable date/name
  list and full scenario details, including workspace and steps. Support empty
  registries, unavailable demos, detail scrolling, and narrow terminals. Keep
  panes side by side at all widths, move IDs to details, and show the description
  before technical metadata.

## [0.1.4] - 2026-10-19

- Restore the original presenter remote's capture-exclusion flag on both native
  windows, including speaker notes. Add a native smoke guard for both flags;
  modern ScreenCaptureKit recorders may still capture these windows.

## [0.1.3] - 2026-10-14

- Show project-prefixed scenario names, descriptions and Warsaw creation times
  in lists and the selector, ordered newest first while preserving run IDs.
- Record creation timestamps automatically during init/registration, preserve
  them across edits, and retain metadata for unavailable scenario files.
- Extend the agent contract with natural-language naming and outcome descriptions.

## [0.1.2] - 2026-10-14

- Add speaker-notes font size controls and left, center, or right alignment,
  preserved across steps and sessions without adding controls to the remote.

## [0.1.1] - 2026-10-14

- Resolve the latest release through the public release redirect, avoiding
  unauthenticated GitHub API rate limits during installation and self-update.

## [0.1.0] - 2026-10-14

- Add a Rust CLI with stable channel IDs and names, exact-ID execution, and an
  interactive selector following the personal CLI configuration/theme interface.
- Add persistent project adapters with verified step completion, operation and
  session limits, read-only previous views, and owned process cleanup.
- Add independent macOS glass windows: a two-button remote and movable,
  resizable speaker notes for a presenter who speaks themselves.
- Include a local browser walkthrough with paced typing, scrolling, highlights,
  role changes, and approval status verification.
- Distribute a macOS Apple Silicon archive, checksum-verified installer, and
  self-update; include the Vibecoding Kernel and agent integration contract.
