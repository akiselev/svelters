// @svelters/test against a mock app that speaks the NDJSON JSON-RPC framing (no Rust build needed),
// and, when SVELTERS_RUN points at a built `svelters-run`, against the real headless host.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { attach, launch, expect, dedent, AutomationError } from '../index.js';

/** A mock app: records requests, answers from `handlers`, replies out of order when asked. */
async function mockApp(handlers) {
  const seen = [];
  const sockets = [];
  const server = net.createServer((s) => {
    sockets.push(s);
    let buf = '';
    s.setEncoding('utf8');
    s.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\n')) >= 0) {
        const req = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        seen.push(req);
        const h = handlers[req.method];
        const reply = (r) => s.write(JSON.stringify({ jsonrpc: '2.0', id: req.id, ...r }) + '\n');
        if (!h) reply({ error: { code: -32601, message: req.method, data: { code: 'method.unknown' } } });
        else Promise.resolve(h(req.params)).then((result) => reply({ result }), (e) => reply({ error: { code: -32000, message: e.message, data: { code: e.code } } }));
      }
    });
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { server, seen, sockets, endpoint: `tcp:${server.address().port}` };
}

test('locators, acts and expect send the app the same methods as Rust', async () => {
  const fail = (code, message) => Object.assign(new Error(message), { code });
  const m = await mockApp({
    'session.hello': () => ({ protocol: 1, roots: [] }),
    'input.click': () => ({ frame: 3, changed: 1 }),
    'input.fill': () => ({ frame: 4 }),
    'input.press': () => ({}),
    'wait.for': (p) => (p.expect?.text === 'nope' ? Promise.reject(fail('timeout', 'waited 50 ms')) : []),
    'wait.settled': () => ({ frames: 1 }),
    'scene.snapshot': () => ({ format: 'yaml', text: '- window "Counter":\n  - label "Count: 1"\n' }),
    // Replies after a later request's reply: the client matches by id.
    'scene.query': () => new Promise((r) => setTimeout(() => r([{ tag: 'button' }, { tag: 'button' }]), 30)),
  });
  const app = await attach(m.endpoint);
  assert.equal(app.hello.protocol, 1);

  const inc = app.root('main').getByRole('button', { name: 'Increment' });
  assert.equal(inc.selector, 'root=main >> role=button[name="Increment"]');
  assert.equal(app.getByText('Count', { exact: true }).nth(1).selector, 'text="Count" >> nth=1');
  assert.equal(app.getByRole('textbox', { name: 'Na', exact: false }).selector, 'role=textbox[name*="Na"]');

  await inc.click({ tier: 'semantic' });
  await app.getByLabel('Name').fill('Ann');
  await app.keyboard.press('Ctrl+A');
  const [count] = await Promise.all([app.locator('button').count(), expect(app.getByText('Count: 1')).toBeVisible()]);
  assert.equal(count, 2);
  await expect(app.getByText('Count')).toHaveText('Count: 1');
  await expect(app).toMatchSceneSnapshot(`
      - window "Counter":
        - label "Count: 1"
  `);

  await assert.rejects(expect(app.getByText('x')).toHaveText('nope'), (e) => e instanceof AutomationError && e.code === 'timeout');
  await assert.rejects(expect(app).toMatchSceneSnapshot('- label "Count: 2"'), (e) => e.code === 'snapshot.mismatch');
  await assert.rejects(app.call('scene.nope'), (e) => e.code === 'method.unknown');

  const byMethod = Object.fromEntries(m.seen.map((r) => [r.method, r.params]));
  assert.deepEqual(byMethod['input.click'], { selector: 'root=main >> role=button[name="Increment"]', tier: 'semantic' });
  assert.deepEqual(byMethod['input.fill'], { selector: 'label="Name"', text: 'Ann' });
  assert.ok(m.seen.every((r) => r.jsonrpc === '2.0' && Number.isInteger(r.id)));

  app.client.close();
  m.server.close();
});

test('a closed app fails pending and later calls with transport.closed', async () => {
  const m = await mockApp({ 'session.hello': () => ({ protocol: 1 }), 'wait.settled': () => new Promise(() => {}) });
  const app = await attach(m.endpoint);
  const pending = app.waitSettled();
  await new Promise((r) => setTimeout(r, 20));
  for (const s of m.sockets) s.destroy();
  m.server.close();
  await assert.rejects(pending, (e) => e.code === 'transport.closed');
  await assert.rejects(app.snapshot(), (e) => e.code === 'transport.closed');
});

test('dedent strips common indentation and blank edges', () => {
  assert.equal(dedent('\n    - a\n      - b  \n  '), '- a\n  - b');
});

// The real host: `SVELTERS_RUN=target/debug/svelters-run pnpm --filter @svelters/test test`.
const bin = process.env.SVELTERS_RUN;
test('the counter fixture through svelters-run', { skip: !bin || !existsSync(bin) ? 'set SVELTERS_RUN to a built svelters-run' : false }, async () => {
  const fixture = fileURLToPath(new URL('../../../fixtures/counter-egui', import.meta.url));
  const app = await launch({ bin, app: fixture, seed: 7, stepMs: 16 });
  try {
    assert.equal(app.hello.app.seed, 7);
    const inc = app.getByRole('button', { name: 'Increment' });
    await inc.click();
    await inc.click({ tier: 'semantic' });
    await expect(app.getByText('Count: 2', { exact: true })).toBeVisible();
    assert.match(await app.snapshot(), /label "Count: 2"/);
    const health = await app.health();
    assert.equal(health.running, null);
  } finally {
    await app.close();
  }
});
