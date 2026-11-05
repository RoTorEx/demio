# Portable demo delivery

A scenario manifest and its adapters are not necessarily a runnable demo on another machine. Portability is part of the delivered workflow when a presenter asks to share it. Keep project-specific runtime bootstrap and data with their project owner; the Demio engine does not reconstruct external application state.

## Required delivery contract

- State the supported operating system, architecture, CLI/native versions, adapter dependencies, live services and authorized account requirements.
- Provide a supported setup entrypoint that resolves the extracted bundle directory, configures destination account/paths explicitly and registers into an isolated config when testing. Never assume the sender's home directory, checkout, Chrome profile, account or ancillary coordination tool exists.
- Preserve existing stopped-project restrictions when applicable. A missing optional coordination tool on another machine is not a reason to manufacture an active state file or copy personal machine state.
- Document interactive sign-in and OS automation permissions. Do not export passwords, tokens, Chrome cookies, a full browser profile or a secret-bearing database dump in an ordinary archive.
- Supply the non-secret fixture/evidence data the scenario actually consumes, or a supported bootstrap that generates it. A read-only presentation of real retained results needs the corresponding real application history. Never replace it with a fabricated proof or silently turn the scenario into a mock.
- For application runtimes, provide exact bootstrap, start, health and stop commands, pinned compatible artifacts, data provenance and the expected verification. Starting a retained-history replay must not launch unrelated workers, paid models or provider mutations.
- Report per-scenario status and unresolved dependencies. A partial source bundle must be labeled partial. A successful doctor is only an adapter-defined prerequisite check, not a complete flow or proof of portable runtime recovery.

## Verification before claiming portability

Extract under a different path with spaces and register with an isolated DEMIO_HOME and DEMIO_CONFIG. Verify those paths before running any replacement registration. Test without source-machine files, old profile metadata or a source checkout. Exercise missing dependency, wrong-account, stopped-state and cleanup behavior. Run doctor for each promised scenario, then rehearse normal interaction and relevant Back/exit transitions against the intended application. State whether this was tested on an actual second machine or only a relocated copy. Never describe one as the other.

The Demio version supports only its documented platform; "another machine" means a supported machine with authorized service access and declared prerequisites, not arbitrary OS/hardware or an account without permission.
