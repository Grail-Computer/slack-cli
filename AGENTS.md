# Grail Slack CLI development

Keep this repository private. Preserve the upstream MIT license and provenance.
This is a private team distribution; do not publish to npm or follow historical
upstream deployment instructions. Read README.md for the user workflow.

Authentication code handles real session credentials. Never log tokens, cookies,
Keychain passwords, or decrypted credential material. Keep credentials out of
process arguments. Cache files must remain owner-only. Live verification should
use auth/read calls; sending or changing Slack data requires user authorization.

For code changes, run `npm test` and syntax-check affected Node modules. Update
README/help when behavior changes. Preserve upstream attribution in UPSTREAM.md.
