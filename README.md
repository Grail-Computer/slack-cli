# Grail Slack CLI

Private Grail distribution of [slkcli](https://github.com/therohitdas/slkcli),
with our macOS authentication fixes included. Use `slk` for agent-driven Slack
access instead of computer/browser automation or the Codex Slack connector.

## Install on your Mac

Prerequisites: Node.js 18+, Git, and the Slack desktop app signed into your own
Grail Computer account. You also need GitHub access to this private repository.

```bash
git clone https://github.com/Grail-Computer/slack-cli.git
cd slack-cli
npm install -g . --ignore-scripts
slk auth
```

If GitHub asks you to authenticate, use your own account (or run `gh auth login`
and `gh auth setup-git` first). Do not use sudo for npm; use a user-owned Node
installation such as nvm if permissions prevent installation.

On first authentication, macOS may ask to access **Slack Safe Storage** in
Keychain. Enter your **Mac login/Keychain password** in the macOS dialog and click
**Always Allow** (sometimes labelled **Always Allow Access**). Do this locally;
never send the password to a person or agent. This is not your Slack password.

Check that `slk auth` shows your own user in **Grail Computer**. Each teammate
uses their own Slack permissions. The CLI caches the session with owner-only
file permissions, so ordinary later commands should not repeat the prompt.
Logout, session revocation, Keychain changes or OS updates may require approval
again; this is not a guarantee of permanent authentication.

## Use

```bash
slk auth
slk read grail-dev 20
slk thread C0636658G9J 1788793776.191449 100
slk search 'in:grail-dev setup'
slk send grail-dev "Your approved message"
slk upload grail-dev /absolute/path/to/file "Your approved caption"
```

Only send, edit, delete, react or upload when the user authorizes it. For an
agent, put the following in your personal/project instructions:

> Use `slk` (Grail Slack CLI) for Slack. Do not use computer/browser automation
> or the official Codex Slack plugin/connector, including as fallbacks. Read
> this repository's README before Slack work. If authentication fails,
> troubleshoot the CLI or report the blocker. Verify the intended workspace and
> user before writes, preserve real newlines, and read messages back afterward.

Full command reference: [upstream README](README.upstream.md).

## Threads and attachments

The stock send/upload commands post to a channel. For thread replies, use
`slackApi` from `src/api.js` with `chat.postMessage` and an explicit `thread_ts`.
For uploads, use `uploadFile` from `src/upload.js` and pass `thread_ts` through
its `slackApi` wrapper for `files.completeUploadExternal`. Never accidentally
post a requested thread reply to the channel root.

Verify replies with `conversations.replies`; verify attachment IDs and sharing
with `files.info`. Slack may normalize email links in returned text. Inspect a
successful post before retrying to avoid duplicates. Follow pagination cursors.

## Updates and troubleshooting

```bash
# Inside this checkout; preserve any local changes first
git pull --ff-only
npm install -g . --ignore-scripts
slk auth
```

If auth fails, check the Slack desktop app is signed in and approve the Keychain
prompt. If the session cache is stale, remove only `~/.local/slk/token-cache.json`
and retry `slk auth`. Never print or share that file: it holds session credentials.
Do not copy another teammate's cache or credentials. Do not reinstall public npm
`slkcli` over this distribution, because it lacks these included changes.

## Grail changes

- Cookie decryption runs inside Node, without an encryption key in process args.
- Auth validation sends credentials to curl on stdin rather than process args.
- Restrictive umask and owner-only token/cookie cache reduce repeated Keychain prompts.
- Private package guard prevents accidental npm publication.

No runtime dependencies. Run `npm test` before changing code. See
[UPSTREAM.md](UPSTREAM.md) for provenance; the original MIT license is retained.
