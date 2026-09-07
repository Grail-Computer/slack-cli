import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const cli = fileURLToPath(new URL('../bin/slk.js', import.meta.url));

test('WhatsApp reads are isolated, paginated, literal, and never mutate the database', () => {
  const dir = mkdtempSync(join(tmpdir(), 'grail-wa-test-'));
  const db = join(dir, 'fixture.sqlite');
  try {
    const setup = spawnSync('python3', ['-c', `import sqlite3,sys
c=sqlite3.connect(sys.argv[1])
c.executescript('''CREATE TABLE ZWACHATSESSION(Z_PK INTEGER,ZCONTACTJID TEXT,ZPARTNERNAME TEXT,ZLASTMESSAGEDATE REAL);
CREATE TABLE ZWAMESSAGE(Z_PK INTEGER,ZCHATSESSION INTEGER,ZMESSAGEDATE REAL,ZISFROMME INTEGER,ZFROMJID TEXT,ZPUSHNAME TEXT,ZTEXT TEXT,ZMESSAGETYPE INTEGER,ZSTANZAID TEXT);
INSERT INTO ZWACHATSESSION VALUES(1,'test@g.us','Fixture',999999999999);
INSERT INTO ZWAMESSAGE VALUES(1,1,0,0,'a','Alice','100% done',0,'a');
INSERT INTO ZWAMESSAGE VALUES(2,1,10,1,'b','Bob','Second',0,'b');''')
c.commit();c.close()`, db], {encoding:'utf8'});
    assert.equal(setup.status, 0, setup.stderr);
    const before = readFileSync(db);
    const run = (...args) => spawnSync(process.execPath, [cli, 'whatsapp', ...args, '--db', db], {encoding:'utf8', env:{...process.env, HOME:dir}});
    assert.equal(JSON.parse(run('chats').stdout).items[0].timestamp,null);
    const page = run('read', 'test@g.us', '--limit', '1');
    assert.equal(page.status,0,page.stderr);
    const p=JSON.parse(page.stdout);
    assert.equal(p.items[0].text,'Second');assert.equal(p.next_offset,1);
    const p2=JSON.parse(run('read','test@g.us','--limit','1','--offset','1').stdout);
    assert.equal(p2.items[0].timestamp,'2001-01-01T00:00:00+00:00');
    assert.equal(p2.has_more,false);
    assert.equal(JSON.parse(run('search','%').stdout).items.length,1);
    assert.equal(JSON.parse(run('search',"' OR 1=1 --").stdout).items.length,0);
    assert.equal(run('read','unknown').status,1);
    assert.equal(run('chats','--limit','0').status,1);
    assert.notEqual(run('send','test@g.us','hello').status,0);
    assert.deepEqual(readFileSync(db),before);
    const missing=join(dir,'missing.sqlite');
    const bad=spawnSync(process.execPath,[cli,'whatsapp','status','--db',missing],{encoding:'utf8',env:{...process.env,HOME:dir}});
    assert.equal(bad.status,1);assert.equal(existsSync(missing),false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
