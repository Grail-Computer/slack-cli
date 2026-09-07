# Working on Slack CLI

Read README.md for installation, authentication, command semantics and limitations.
This repository is public. Keep real messages, cookies, tokens, databases and
Keychain material out of code, fixtures, issues, logs and release artifacts.

Use the native `slk` executable for Slack operations. Verify `slk auth` before
writes; sending, editing, deleting, reactions and uploads require the user's
explicit authorization. Use `--text-file` / `--caption-file` for multiline text,
`--thread` for replies, and read back writes before retrying an uncertain result.

Authentication uses the macOS Keychain API and a protected session cache.
Preserve read-only app database access, owner-only cache permissions, bounded
LevelDB parsing and credential-free diagnostics. Read commands must not create DMs.

Run `cargo test --locked`, `cargo fmt --check`, and
`cargo clippy --locked --all-targets -- -D warnings` after changes.
Use synthetic fixtures; live smoke checks should output only counts/booleans.
Run scripts/package.sh on macOS for releases; retain LICENSE and UPSTREAM.md.
