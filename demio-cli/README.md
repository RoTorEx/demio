# demio-cli

Rust component of the Demio product. Its public terminal command is `demio`.
It owns the TUI, demo/scenario loading, configuration, adapter protocol, sequencing
and cleanup. It launches the sibling demio-overlay executable for native controls.
Build and verify both components from the product root with `make check`; package
and release them together. The root AGENTS.md and BUSINESS.md own project rules
and product contracts.
