#!/usr/bin/env node
// A stand-in for svelters-run that speaks the NDJSON JSON-RPC framing (runner self-tests without a
// Rust build). Its "app" is a counter: `input.click` counts, `wait.for` checks a text against the
// labels `Count: N`, `Seed: S` and `Title: T` (a mounted component's `title` prop). It appends
// `launch <seed>`, `reset <seed>` and `mount <seed>` lines to `$MOCK_LOG`. `app.crash` exits;
// `app.hang` never answers.

import net from 'node:net';
import { appendFileSync } from 'node:fs';

const argv = process.argv.slice(2);
const flag = (name) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : undefined;
};
let seed = Number(flag('--seed') ?? 0);
let count = 0;
let props = null;
let clockMs = 0;
const log = (line) => process.env.MOCK_LOG && appendFileSync(process.env.MOCK_LOG, `${line}\n`);
log(`launch ${seed}`);

const fail = (code, message) => ({ error: { code: -32000, message, data: { code } } });
const labels = () => [`Count: ${count}`, `Seed: ${seed}`, ...(props ? [`Title: ${props.title}`] : [])];

const methods = {
  'session.hello': () => ({ protocol: 1, roots: [], grants: { read: true, act: true, tooling: false, clock: true }, app: { headless: true, clock: 'virtual', seed, mount: true } }),
  'app.reset': (p) => {
    seed = p.seed ?? Number(flag('--seed') ?? 0);
    count = 0;
    props = null;
    clockMs = 0;
    log(`reset ${seed}`);
    return { seed, frames: 1 };
  },
  'app.mount': (p) => {
    seed = p.seed ?? seed;
    count = 0;
    clockMs = 0;
    props = { ...p.props };
    log(`mount ${seed} ${p.module.split('/').pop()}`);
    return { root: 'mount', seed, frames: 1 };
  },
  'app.set_props': (p) => {
    if (!props) return fail('capability.absent', 'nothing is mounted');
    Object.assign(props, p.props);
    return { frames: 1 };
  },
  'input.click': () => {
    count++;
    return { frame: 1 };
  },
  // `text="…"` (exact) or `text=…` (contains) in the selector, or `expect.text`.
  'wait.for': (p) => {
    const m = /text=(".*"|.*)$/.exec(p.selector ?? '');
    const [want, exact] = m ? (m[1].startsWith('"') ? [JSON.parse(m[1]), true] : [m[1], false]) : [p.expect?.text, p.expect?.exact];
    if (want === undefined || labels().some((l) => (exact ? l === want : l.includes(want)))) return [];
    return fail('timeout', `no label ${JSON.stringify(want)} (have ${labels().join(', ')})`);
  },
  'wait.settled': () => ({ frames: 1 }),
  'clock.now': () => ({ mode: 'virtual', ms: clockMs, seed, session_ms: clockMs }),
  'clock.advance': (p) => {
    clockMs += p.ms;
    return { frames: Math.ceil(p.ms / 16), ms: p.ms };
  },
};

const server = net.createServer((s) => {
  let buf = '';
  s.setEncoding('utf8');
  s.on('error', () => {});
  s.on('data', (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const req = JSON.parse(buf.slice(0, i));
      buf = buf.slice(i + 1);
      const reply = (r) => s.write(JSON.stringify({ jsonrpc: '2.0', id: req.id, ...r }) + '\n');
      if (req.method === 'app.exit') {
        reply({ result: {} });
        setTimeout(() => process.exit(0), 10);
      } else if (req.method === 'app.crash') process.exit(3);
      else if (req.method === 'app.hang') continue;
      else {
        const h = methods[req.method];
        const r = h ? h(req.params ?? {}) : fail('method.unknown', req.method);
        reply(r?.error ? r : { result: r });
      }
    }
  });
});
server.listen(0, '127.0.0.1', () => console.log(`SVELTERS_AUTOMATION=tcp:${server.address().port}`));
