# Contributing to Folk Craft

Thank you for your interest. Bug reports, fixes, tests, documentation and new content are all welcome.

## Getting started

1. Install [Rust](https://www.rust-lang.org/tools/install) (stable). On Debian or Ubuntu also install `libasound2-dev`.
2. Fork the repository and create a branch from `main`.
3. Read [`AGENTS.md`](AGENTS.md): it is the technical guide (code map, conventions and invariants, how to extend).

## Before you open a pull request

```sh
cargo build    # must produce no warnings
cargo test     # no window or GPU needed
```

- Keep logic in plain functions and methods that can be unit tested; keep macroquad drawing and input in the thin UI
  wrappers described in `AGENTS.md`.
- Add or update tests for new logic, and update the in-game Help (`src/help.rs`) and `README.md` when rules change.
- Do not add dependencies without a strong reason.
- Keep commits focused, with a short imperative subject line ("Fix crow altar lookup").
- By submitting a contribution you agree that it is licensed under the [MIT License](LICENSE).

## Reporting bugs and requesting features

Use the issue templates. Include the commit, your operating system, and steps to reproduce. Report security problems
privately, as described in [`SECURITY.md`](SECURITY.md).

All participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).
