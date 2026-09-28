# Linux agent CLI

This directory carries the Linux build of the Grail Slack CLI alongside the
macOS Rust implementation at the repository root. It is based on
`github.com/howar31/slk` v0.8.1 and includes the Grail `_Sent by Agent_` footer
for outgoing messages, thread replies, scheduled messages, drafts, ephemeral
messages, updates, and generic message API calls.

Build on Linux with Go 1.25+:

```sh
cd linux/slk
go test ./...
go build -trimpath -ldflags='-s -w' -o slk ./cmd/slk
```

The Linux CLI uses its own Slack user OAuth token and does not read macOS
Slack Desktop or Keychain state. Keep tokens outside this repository.
