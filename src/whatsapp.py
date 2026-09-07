"""Read-only adapter for the native WhatsApp macOS Core Data store."""
from contextlib import closing
import argparse
import datetime as dt
import json
from pathlib import Path
import sqlite3
import sys

DEFAULT_DB = Path.home() / 'Library/Group Containers/group.net.whatsapp.WhatsApp.shared/ChatStorage.sqlite'
EPOCH = 978307200


def timestamp(value):
    if value is None:
        return None
    try:
        return dt.datetime.fromtimestamp(value + EPOCH, dt.timezone.utc).isoformat()
    except (ValueError, OverflowError, OSError):
        # WhatsApp uses out-of-range sentinel dates for some chat records.
        return None


def query(path, command, value=None, limit=50, offset=0, chat=None):
    path = Path(path).expanduser().resolve()
    if not path.is_file():
        raise ValueError('WhatsApp database not found. Install and sign into the native WhatsApp Mac app first.')
    if not 1 <= limit <= 500 or offset < 0:
        raise ValueError('limit must be 1–500; offset must be nonnegative')
    with closing(sqlite3.connect(path.as_uri() + '?mode=ro', uri=True, timeout=5)) as conn:
        conn.row_factory = sqlite3.Row
        conn.execute('PRAGMA query_only=ON')
        required = {
            'ZWACHATSESSION': {'Z_PK', 'ZCONTACTJID', 'ZPARTNERNAME', 'ZLASTMESSAGEDATE'},
            'ZWAMESSAGE': {'Z_PK', 'ZCHATSESSION', 'ZMESSAGEDATE', 'ZISFROMME', 'ZFROMJID', 'ZPUSHNAME', 'ZTEXT', 'ZMESSAGETYPE', 'ZSTANZAID'},
        }
        for table, columns in required.items():
            found = {r[1] for r in conn.execute('PRAGMA table_info(' + table + ')')}
            if not columns <= found:
                raise ValueError('Unsupported WhatsApp database schema; update the CLI adapter.')
        if command == 'status':
            return {'source': 'whatsapp-macos', 'read_only': True, 'database': str(path),
                    'chats': conn.execute('SELECT COUNT(*) FROM ZWACHATSESSION').fetchone()[0],
                    'messages': conn.execute('SELECT COUNT(*) FROM ZWAMESSAGE').fetchone()[0],
                    'latest_local_message_at': timestamp(conn.execute('SELECT MAX(ZMESSAGEDATE) FROM ZWAMESSAGE').fetchone()[0]),
                    'sending_supported': False}
        if command == 'chats':
            sql = 'SELECT ZCONTACTJID AS id, ZPARTNERNAME AS name, ZLASTMESSAGEDATE AS timestamp FROM ZWACHATSESSION'
            args = []
            if value:
                sql += " WHERE instr(lower(COALESCE(ZPARTNERNAME,'')),lower(?)) > 0 OR instr(COALESCE(ZCONTACTJID,''),?) > 0"
                args += [value, value]
            sql += ' ORDER BY ZLASTMESSAGEDATE DESC, Z_PK DESC LIMIT ? OFFSET ?'
        else:
            sql = '''SELECT m.Z_PK AS local_id, m.ZSTANZAID AS message_id, c.ZCONTACTJID AS chat_id,
                c.ZPARTNERNAME AS chat_name, m.ZMESSAGEDATE AS timestamp, m.ZISFROMME AS from_me,
                m.ZFROMJID AS sender_id, m.ZPUSHNAME AS sender_name, m.ZTEXT AS text,
                m.ZMESSAGETYPE AS message_type
                FROM ZWAMESSAGE m JOIN ZWACHATSESSION c ON c.Z_PK=m.ZCHATSESSION WHERE 1=1'''
            args = []
            if command == 'read':
                if not value:
                    raise ValueError('read requires an exact chat ID from chats')
                if not conn.execute('SELECT 1 FROM ZWACHATSESSION WHERE ZCONTACTJID=?', (value,)).fetchone():
                    raise ValueError('Chat ID not found; use whatsapp chats to find an exact ID.')
                sql += ' AND c.ZCONTACTJID=?'
                args.append(value)
            elif command == 'search':
                if not value:
                    raise ValueError('search requires a nonempty query')
                sql += " AND instr(lower(COALESCE(m.ZTEXT,'')),lower(?)) > 0"
                args.append(value)
                if chat:
                    sql += ' AND c.ZCONTACTJID=?'
                    args.append(chat)
            else:
                raise ValueError('Unsupported WhatsApp command')
            sql += ' ORDER BY m.ZMESSAGEDATE DESC, m.Z_PK DESC LIMIT ? OFFSET ?'
        rows = [dict(r) for r in conn.execute(sql, (*args, limit + 1, offset))]
        more = len(rows) > limit
        rows = rows[:limit]
        for row in rows:
            row['timestamp'] = timestamp(row['timestamp'])
            if 'from_me' in row:
                row['from_me'] = bool(row['from_me'])
        return {'source': 'whatsapp-macos', 'order': 'newest_first', 'items': rows,
                'has_more': more, 'next_offset': offset + limit if more else None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['status', 'chats', 'read', 'search'])
    parser.add_argument('value', nargs='?')
    parser.add_argument('--db', default=str(DEFAULT_DB))
    parser.add_argument('--limit', type=int, default=50)
    parser.add_argument('--offset', type=int, default=0)
    parser.add_argument('--chat')
    args = parser.parse_args()
    try:
        print(json.dumps(query(args.db, args.command, args.value, args.limit, args.offset, args.chat), ensure_ascii=False))
    except (sqlite3.Error, ValueError, OSError) as exc:
        print(json.dumps({'error': str(exc), 'hint': 'For permission errors, allow the invoking terminal/Codex access in macOS Privacy settings. No database was modified.'}), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
