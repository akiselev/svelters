// DX2: `svelters:clipboard` and `svelters:shell` over a fake `__svelters_host` (the ops of
// svelters-host `quickjs/platform.rs`). crates/svelters-egui/tests/clipboard.rs runs the same modules
// on QuickJS and egui.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { clipboard, read, readAll, formats, write, readText } from '../src/clipboard.js';
import { openPath, openUrl, revealInFolder } from '../src/shell.js';

const matches = (p, t) => p === '*' || p === t || (p.endsWith('/*') && t.startsWith(p.slice(0, -1)));
const boards = { clipboard: [], primary: [] };
const calls = [];
globalThis.__svelters_host = {
  clipboardWrite(kind, entries) {
    boards[kind] = entries.map(([t, v]) => [t, v]);
  },
  clipboardRead(kind, accept) {
    return boards[kind].filter(([t]) => accept.length === 0 || accept.some((p) => matches(p, t)));
  },
  clipboardFormats(kind) {
    return boards[kind].map(([t]) => t);
  },
  shellOpen(what, target) {
    if (!target.startsWith('/')) throw new Error(`invalid shell argument: ${target}`);
    calls.push([what, target]);
  },
};

test('write encodes strings, bytes and JSON; read picks by preference', async () => {
  await write({ 'text/plain': 'a\tb', 'image/png': new Uint8Array([1, 2]).buffer, 'application/x-refs+json': ['R1'] });
  assert.deepEqual(await formats(), ['text/plain', 'image/png', 'application/x-refs+json']);
  assert.deepEqual(boards.clipboard[1][1], new Uint8Array([1, 2]));
  const e = await read(['image/jpeg', 'application/*', 'text/plain']);
  assert.equal(e.type, 'application/x-refs+json');
  assert.deepEqual(e.json(), ['R1']);
  assert.equal((await read()).type, 'text/plain');
  assert.equal(await read('text/html'), null);
  assert.deepEqual(Object.keys(await readAll('text/*')), ['text/plain']);
});

test('the primary selection is separate; failures reject', async () => {
  await clipboard.writeText('sel', { kind: 'primary' });
  assert.equal(await readText({ kind: 'primary' }), 'sel');
  assert.equal(await readText(), 'a\tb');
  await assert.rejects(write(null), TypeError);
  await assert.rejects(write({ 'text/plain': undefined }), TypeError);
});

test('shell calls reach the host and reject on its errors', async () => {
  await openPath('/tmp/a');
  await revealInFolder('/tmp/b');
  await openUrl('/u');
  assert.deepEqual(calls, [['path', '/tmp/a'], ['reveal', '/tmp/b'], ['url', '/u']]);
  await assert.rejects(openPath('rel'), /invalid shell argument/);
});
