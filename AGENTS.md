# Working on Slack CLI

Read README.md for installation, authentication, command semantics and limitations.
When setting up an agent, also install `skills/grail-slack-cli` into
`$HOME/.agents/skills/grail-slack-cli` as described in the README.
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

## Linux work

`main` is the macOS Rust build only. Do Linux work on the `linux-agent-footer`
branch, in the Go CLI under `linux/slk` (based on `github.com/howar31/slk`
v0.8.1). Commit and push Linux changes to that branch; do not merge it into
`main` or port Linux code into the Rust sources.

Every command that sends or edits message text must pass it through
`withAgentFooter` in `linux/slk/internal/commands/input.go`, as `msg.go`,
`thread.go` and `api.go` do, so Linux messages carry the same `_Sent by Agent_`
footer as macOS. Cover new call sites in the package tests.

Run `go test ./...` and `go build -trimpath -ldflags='-s -w' -o slk ./cmd/slk`
from `linux/slk` (Go 1.25+). The Linux CLI uses a Slack OAuth token stored with
`slk auth set-token`; it never reads Slack Desktop or Keychain state.
