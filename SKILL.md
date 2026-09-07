---
name: grail-slack-cli
description: Read Slack threads, search messages, and perform user-authorized Slack actions through Grail's local slk CLI.
---

# Grail Slack CLI

Read README.md for installation, authentication, commands and thread/file handling.
Use this private Grail distribution, not the public npm package. Authenticate
with `slk auth` and verify the intended workspace and user. Use CLI access only;
do not fall back to browser/computer control or the Codex Slack connector.

The human enters their Mac login/Keychain password into the native Slack Safe
Storage dialog and chooses Always Allow. Never request or record that password.
Protect the owner-only session cache; never print or share credentials.

Read the entire requested thread, following pagination where needed. Perform
external mutations only within the user's authorization. Preserve real newlines
and verify posted text, thread and attachment IDs by reading them back. If a
post succeeds but verification fails, inspect it before retrying.
