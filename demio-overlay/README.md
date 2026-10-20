# demio-overlay

Swift/AppKit component of the Demio product. It owns the native presenter window,
narration rendering, font/alignment controls and global F7/F9 hotkeys. It consumes
the CLI's session display file and sends navigation commands; it does not execute
project adapters. It is launched and cleaned up by demio-cli, and is installed
next to the `demio` command. Build from the product root with `make build`; run
`make smoke-ui` on the local desktop for native checks. Both components share
one product version and release bundle.

Presenter preferences belong to the shared config.toml [overlay] section. This
component uses the sibling demio executable's internal --overlay-settings
interface to read/write them, preserving the CLI's registry and theme. Both
components must be installed together. The CLI owns legacy JSON migration.
