# Slack CLI for macOS

**Slack from your terminal or coding agent. One Rust executable.**

`slk` reads conversations, searches messages, replies to threads, uploads files,
and manages drafts using the account already signed into Slack Desktop.
No Node.js, Python, Homebrew, Rust toolchain, or database installation is needed
to run a release binary.

[Download the latest release](https://github.com/Grail-Computer/slack-cli/releases/latest)
· [WhatsApp CLI](https://github.com/Grail-Computer/whatsapp-cli)
· [Command guide for agents](AGENTS.md)

## Install

Requires macOS 11 or newer and Slack Desktop signed in. Supports both Apple
Silicon (M-series) and Intel Macs. This project is independent of Slack; it is
not an official Slack SDK or the `slack` app-development CLI.

1. Open the release page and download `slk-macos-arm64.tar.gz` for Apple Silicon,
   or `slk-macos-x86_64.tar.gz` for Intel. **About This Mac** shows your chip.
2. Extract the archive. It contains `slk`, this README, and the license.
3. In Terminal, install the extracted executable:

```sh
mkdir -p "$HOME/.local/bin"
# Run this in the folder containing the extracted slk file:
install -m 755 ./slk "$HOME/.local/bin/slk"
export PATH="$HOME/.local/bin:$PATH"
slk --version
slk auth
```

Add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc` once to keep it available
in new terminals. Existing npm users should check `which -a slk` and put
`~/.local/bin` first in PATH. The Rust version can reuse the existing session
cache. The old npm installation is no longer required.

The downloads are ad-hoc signed, **not Apple-notarized**. macOS may block a
browser download. After verifying the release and checksum, allow that specific
binary in System Settings → Privacy & Security. If macOS still quarantines the
file, `xattr -d com.apple.quarantine "$HOME/.local/bin/slk"` removes quarantine
from that file only; do not disable Gatekeeper globally.

`SHA256SUMS` accompanies each release. Verify the downloaded archive with
`shasum -a 256 slk-macos-arm64.tar.gz` (or the Intel filename) and compare it
with the matching line in `SHA256SUMS` before installing.

## First authentication

On first use, macOS may ask for access to **Slack Safe Storage**. Enter your
**Mac login / Keychain password** in the native dialog and click **Always Allow**
(or **Always Allow Access**). This is not your Slack password. Never paste it
into a terminal command, agent chat, or issue.

`slk auth` reports the user and workspace; verify these before sending anything.
Commands act with that user's permissions. The CLI validates its cached session
on each run and refreshes it from Slack Desktop when necessary. Keychain changes,
logout, revoked sessions, or a changed executable can require approval again.
With multiple signed-in workspaces, selection follows the first valid desktop
session found; use `auth` to check it. There is no workspace selector in v1.

```sh
slk auth
slk auth --refresh  # Re-read the desktop session and Keychain
```

Credentials are cached at `~/.local/slk/token-cache.json` (file mode `0600`,
directory `0700`). Never share or commit this file. No session credentials are
printed or passed to subprocesses. Requests use HTTPS directly from the binary.
The Slack cookie database is opened read-only; the CLI does not alter app files.

## Everyday commands

All successful commands print JSON to stdout. Errors print JSON to stderr and
exit nonzero; help/argument errors use the normal CLI help format. Output may
contain private conversations, so protect redirected files accordingly.

```sh
slk channels
slk dms
slk users
slk read general 20
slk read @alex 100 --threads
slk read general 100 --from 2026-01-01 --to 2026-02-01
slk thread CHANNEL_ID THREAD_TIMESTAMP 100
slk search 'in:general "release"' 20
slk search 'in:general "release"' 20 --page 2
slk unread
slk activity
slk pins general
slk starred
slk saved 20
slk saved 20 --all
```

Channels accept a name, `#name`, or exact channel ID. DMs accept `@username` or a
user ID. Ambiguous names fail; use an exact ID. A read command never creates a
new DM. `read` returns newest first; `thread` returns oldest first, including the
parent. Date boundaries use UTC: `--from` is inclusive and `--to` exclusive.
Search queries containing spaces must be quoted; search supports up to 100
results per page.

List/read/thread commands follow Slack cursors until the requested count is
reached. If `has_more` is true, use `next_cursor` with `read --cursor` or
`thread --cursor`; use `api` for other cursor-based endpoints. Search returns
Slack's page metadata. A non-cursor `has_more` response requires inspecting the
raw API response and continuing with that endpoint's pagination parameters.
`--threads` expands each returned parent, potentially making many requests.
Slack rate limits are reported with the retry interval; writes are never retried
automatically because doing so could duplicate a message.

## Send messages and files

```sh
slk send general "Build passed"
slk send CHANNEL_ID --thread THREAD_TIMESTAMP --text-file reply.txt
slk upload CHANNEL_ID report.pdf "Review copy"
slk upload CHANNEL_ID report.pdf --thread THREAD_TIMESTAMP --caption-file caption.txt
slk react CHANNEL_ID MESSAGE_TIMESTAMP eyes
```

Text files preserve real newlines. `--thread` keeps replies and attachments in
the intended thread. After posting, read the thread back and verify the message
or attachment ID; Slack may normalize links in the returned text. Inspect a
successful or uncertain response before retrying.

## Drafts

```sh
slk draft general "Review this before sending"
slk draft thread CHANNEL_ID THREAD_TIMESTAMP "Proposed reply"
slk draft user USER_ID "Proposed DM"
slk drafts
slk draft drop DRAFT_ID
```

Drafts sync into Slack's UI. Drafts, saved items, preferences, and activity use
internal Slack APIs that can change or be unavailable for some accounts. Such
failures are reported explicitly; the CLI does not silently substitute a send.

## Direct API access

Use `slk api` when you need a Slack endpoint that has no dedicated command.
Parameters come from a JSON object in a file, preserving structured fields and
newlines. This command can perform writes; review the method and parameters.

```sh
slk api auth.test
slk api conversations.replies --params-file request.json
```

Example `request.json`:

```json
{"channel":"CHANNEL_ID","ts":"THREAD_TIMESTAMP","limit":100}
```

Use `files.info` to inspect attachment metadata and sharing. File downloads are
not a dedicated v1 command. See `slk --help` and `slk COMMAND --help` for options.
Aliases from the earlier CLI remain (`ch`, `dm`, `u`, `r`, `t`, `s`, `a`, `ur`,
`star`, `sv`, `pin`). Output changed from decorated text to JSON. WhatsApp is now
[its own CLI](https://github.com/Grail-Computer/whatsapp-cli), `wacli`.

## Troubleshooting

- **Wrong account or expired session:** sign into the intended Slack Desktop
  account, then run `slk auth --refresh`. v1 has no account selector.
- **Keychain denied:** retry refresh and approve the native dialog. Do not copy
  another person's cache or credentials.
- **Desktop data not found:** use the native Slack Mac app. Direct-download and
  Mac App Store locations are supported; browser-only login is insufficient.
- **Permission denied:** check access to Slack app data for your terminal/agent.
- **Rate limited:** wait the interval shown, then retry the read. Check whether
  an uncertain write succeeded before resending.
- **Method/schema changed:** report the error and app version without tokens,
  cookies, private messages, or database files. Internal APIs are unofficial.
- **Old version still runs:** inspect `which -a slk` and correct PATH ordering.

## Build and verify from source

End users only need the download. Contributors need Rust 1.92+ and Xcode Command
Line Tools. SQLite and TLS are linked into the executable; dynamic dependencies
are macOS system libraries/frameworks. No helper executables run at runtime.

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
./target/release/slk --help
./scripts/package.sh
```

The packaging script builds Apple Silicon and Intel archives in `dist/`, with
SHA-256 checksums. CI runs tests/lint and builds both targets; live authentication
requires your local Slack session and is never run with CI secrets.

We bundle SQLite rather than using the Rust Turso engine because these are live
SQLite files owned by another process. Turso's [compatibility guide](https://github.com/tursodatabase/turso/blob/main/COMPAT.md)
explicitly excludes mixed SQLite/Turso multi-process access. This choice adds no
installation dependency for users.

## License and provenance

MIT. Rust implementation maintained by Grail Computer, based on the workflows
and authentication approach of Rohit Das's [slkcli](https://github.com/therohitdas/slkcli).
Original license and source history are retained; see [UPSTREAM.md](UPSTREAM.md).
Slack is a trademark of its owner; this project is not affiliated with Slack.
