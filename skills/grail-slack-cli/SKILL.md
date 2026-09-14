---
name: grail-slack-cli
description: Use the Grail Slack CLI (`slk`) for Slack reads, searches, threads, approved writes, edits, and file uploads on macOS. Apply whenever an agent needs Slack access in Grail Computer.
---

# Grail Slack CLI

Use the native Grail Slack CLI for Slack. Do not use browser/computer automation or the official Codex Slack connector, including as fallbacks. Read the repository README at https://github.com/Grail-Computer/slack-cli when setup or command details are unclear.

## Before using Slack

- Run `slk auth` and verify the reported user and workspace. The default workspace is **Grail Computer**.
- Select a different signed-in workspace only when needed: `slk --workspace "Workspace Name" auth --refresh` or use its team ID.
- Each person uses their own Slack Desktop session and Keychain. Never request, print, copy, or share cookies, tokens, Keychain passwords, or the session cache.

## Reading and searching

Use `slk channels`, `slk dms`, and `slk users` to discover accessible resources. Use `slk read CHANNEL COUNT` for history, `slk read CHANNEL COUNT --threads` to expand replies, and `slk thread CHANNEL_ID THREAD_TS COUNT` for one thread. Use `slk search 'QUERY' COUNT` for workspace search. Access is limited to the authenticated account's Slack permissions.

## Sending, editing, and files

- Sending, editing, deleting, reacting, and uploading are mutations. Do them only when the user explicitly authorizes the action.
- `slk send` appends the italic Slack footer `Sent by Agent`. Inline literal `\\n` sequences become real line breaks; `--text-file` preserves the file exactly.
- For reliable rich formatting during edits or advanced posts, use `slk api chat.update` or `slk api chat.postMessage` with a params JSON file containing explicit Slack `blocks` rich-text elements. Give link elements a visible `text` label and include the italic footer in the blocks when supplying blocks directly.
- Use `slk upload CHANNEL FILE [CAPTION]` for images and other files. Clipboard content must first be available as a local file; the CLI does not read the macOS clipboard directly.
- After every successful write, read the message or thread back and verify its timestamp, content, formatting blocks, or file attachment before retrying.

## Setup and failures

Install the release binary or build from this repository as described in the README. Install this skill globally as part of setup with:

```sh
mkdir -p "$HOME/.agents/skills"
cp -R skills/grail-slack-cli "$HOME/.agents/skills/grail-slack-cli"
```

If authentication, permissions, rate limits, or Slack's internal API schema fail, report the concrete CLI error and stop before retrying a mutation. Never substitute another Slack integration.
