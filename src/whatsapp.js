import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export function whatsapp(args) {
  if (!args.length || args[0] === '--help' || args[0] === 'help') {
    console.log(`WhatsApp Mac access (local read-only JSON):
  slk whatsapp status
  slk whatsapp chats [name] [--limit 50] [--offset 0]
  slk whatsapp read <exact-chat-id> [--limit 50] [--offset 0]
  slk whatsapp search <text> [--chat <exact-chat-id>] [--limit 50]

Requires Python 3 and the signed-in native WhatsApp Mac app.
Reads locally synced history only. No sending, UI automation, or database writes.
Results are newest-first; use next_offset to retrieve another page.
An optional --db PATH selects a different compatible SQLite database.`);
    return;
  }
  const result = spawnSync('python3', [fileURLToPath(new URL('./whatsapp.py', import.meta.url)), ...args], {
    stdio: 'inherit', timeout: 30000,
  });
  if (result.error) {
    console.error(result.error.code === 'ENOENT' ? 'Python 3 is required for WhatsApp access.' : 'WhatsApp reader failed or timed out.');
    process.exitCode = 1;
  } else {
    process.exitCode = result.status ?? 1;
  }
}
