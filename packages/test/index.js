// @svelters/test (AU2, E2E-INSPECTOR §2.2): the TypeScript side of svelters-automation's socket
// transport. It sends `Session::call`'s methods unchanged as NDJSON JSON-RPC 2.0, so a test here
// and a Rust test in process run the same requests.
//
//   const app = await launch({ bin: 'target/debug/svelters-run', app: 'fixtures/counter-egui', seed: 7 });
//   await app.getByRole('button', { name: 'Increment' }).click();
//   await expect(app.getByText('Count: 1', { exact: true })).toBeVisible();
//   await expect(app.root('main')).toMatchSceneSnapshot(`- label "Count: 1"`);
//   await app.close();
//
// Waits run in the app (`wait.for`, evaluated after each settle, timeouts on the app's virtual
// clock), so `expect` sends one request per assertion and never polls or sleeps.
//
// AU2b: `app.clock` (`clock.now`/`clock.advance`, the `clock` grant), `app.reset()` and
// `app.mount()` (a component alone in a fresh host on a fresh virtual clock), and the runner
// (`test`, the `svelters-e2e` CLI: runner/). Not here yet: `toHaveScreenshot` (needs pixels over
// the wire). See plans/milestones/AU2b-ts-runner-mount.md.

import net from 'node:net';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { resolve as resolvePath } from 'node:path';
import { fileURLToPath } from 'node:url';

/** A protocol error: `code` is the app's stable code (`node.not_found`, `timeout`, …). */
export class AutomationError extends Error {
  constructor(code, message, method) {
    super(`${code}: ${message}${method ? ` (${method})` : ''}`);
    this.name = 'AutomationError';
    this.code = code;
  }
}

/** `unix:/path`, a bare path, or `tcp:PORT` (loopback). */
export function parseEndpoint(endpoint) {
  if (endpoint.startsWith('tcp:')) {
    const port = Number(endpoint.slice(4));
    if (!Number.isInteger(port)) throw new AutomationError('transport.address', `${endpoint}: want tcp:PORT`);
    return { host: '127.0.0.1', port };
  }
  return { path: endpoint.startsWith('unix:') ? endpoint.slice(5) : endpoint };
}

/** One connection. Calls may overlap; replies are matched by id. */
export class Client {
  #socket;
  #next = 1;
  #pending = new Map();
  #closed = null;

  constructor(socket) {
    this.#socket = socket;
    socket.setEncoding('utf8');
    const lines = createInterface({ input: socket, crlfDelay: Infinity });
    lines.on('line', (line) => {
      if (!line.trim()) return;
      let msg;
      try {
        msg = JSON.parse(line);
      } catch {
        return;
      }
      const p = this.#pending.get(msg.id);
      if (!p) return;
      this.#pending.delete(msg.id);
      if (msg.error) p.reject(new AutomationError(msg.error.data?.code ?? 'rpc.error', msg.error.message, p.method));
      else p.resolve(msg.result);
    });
    const close = (why) => {
      this.#closed ??= why;
      for (const p of this.#pending.values()) p.reject(new AutomationError('transport.closed', why, p.method));
      this.#pending.clear();
    };
    socket.on('close', () => close('the app closed the connection'));
    socket.on('error', (e) => close(e.message));
  }

  /** Connects to `endpoint`, retrying until `timeout` ms while the app starts. */
  static async connect(endpoint, { timeout = 10_000 } = {}) {
    const opts = parseEndpoint(endpoint);
    const deadline = Date.now() + timeout;
    for (;;) {
      try {
        const socket = await new Promise((resolve, reject) => {
          const s = net.createConnection(opts, () => resolve(s));
          s.once('error', reject);
        });
        return new Client(socket);
      } catch (e) {
        if (Date.now() >= deadline) throw new AutomationError('transport.io', `${endpoint}: ${e.message}`);
        await new Promise((r) => setTimeout(r, 20));
      }
    }
  }

  /** Sends `method` and resolves with its result (rejects with an `AutomationError`). */
  call(method, params = {}) {
    if (this.#closed) return Promise.reject(new AutomationError('transport.closed', this.#closed, method));
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject, method });
      this.#socket.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }

  close() {
    this.#socket.end();
  }
}

const quote = (s) => JSON.stringify(String(s));
const chain = (scope, sel) => (scope ? `${scope} >> ${sel}` : sel);

/** A lazy selector: resolved in the app on every action, strictly (one node) for acts. */
export class Locator {
  constructor(app, selector) {
    this.app = app;
    this.selector = selector;
  }

  locator(selector) {
    return new Locator(this.app, chain(this.selector, selector));
  }
  getByRole(role, { name, exact = true } = {}) {
    return this.locator(name === undefined ? `role=${role}` : `role=${role}[name${exact ? '' : '*'}=${quote(name)}]`);
  }
  getByText(text, { exact = false } = {}) {
    return this.locator(exact ? `text=${quote(text)}` : `text=${text}`);
  }
  getByLabel(label) {
    return this.locator(`label=${quote(label)}`);
  }
  nth(i) {
    return this.locator(`nth=${i}`);
  }
  first() {
    return this.nth(0);
  }

  #act(method, extra, opts = {}) {
    const { tier, force, timeout } = opts;
    return this.app.call(method, { selector: this.selector, ...extra, tier, force, timeout });
  }
  click(opts) {
    return this.#act('input.click', {}, opts);
  }
  hover(opts) {
    return this.#act('input.hover', {}, opts);
  }
  focus(opts) {
    return this.#act('input.focus', {}, opts);
  }
  fill(text, opts) {
    return this.#act('input.fill', { text }, opts);
  }
  /** `opts.handle`: one handle of a multi-handle widget (`'low'`/`'high'` of a range-slider). */
  setValue(value, opts = {}) {
    return this.#act('input.set_value', { value, handle: opts.handle }, opts);
  }

  /** Every node that matches now (no waiting). */
  async all() {
    return this.app.call('scene.query', { selector: this.selector });
  }
  async count() {
    return (await this.all()).length;
  }
  /** The one node (strict). */
  async info() {
    return this.app.call('scene.node', { selector: this.selector });
  }
  async textContent() {
    return (await this.info()).text ?? null;
  }
  /** The scene snapshot (aria-snapshot YAML) of this locator's nodes. */
  async snapshot() {
    return (await this.app.call('scene.snapshot', { selector: this.selector })).text;
  }
}

/** A running app (from `launch`) or a connection to one (`attach`). */
export class App extends Locator {
  constructor(client, { child = null, endpoint, hello } = {}) {
    super(null, '');
    this.app = this;
    this.client = client;
    this.child = child;
    this.endpoint = endpoint;
    this.hello = hello;
    this.keyboard = {
      press: (key) => this.call('input.press', { key }),
      type: (text) => this.call('input.type', { text }),
    };
    /**
     * The app's clock (AU1b). `now()` is `clock.now` (`{mode, ms, seed, session_ms}`);
     * `advance(ms)`/`runFor(ms)` let `ms` of virtual time pass frame by frame (timers fire,
     * transitions finish) and need the session's `clock` grant and a virtual clock.
     */
    this.clock = {
      now: () => this.call('clock.now'),
      advance: (ms) => this.#clockGrant().then(() => this.call('clock.advance', { ms })),
      runFor: (ms) => this.clock.advance(ms),
    };
  }

  /** The session's grants (`read`, `act`, `tooling`, `clock`), from `session.hello`. */
  get grants() {
    return this.hello?.grants ?? {};
  }

  async #clockGrant() {
    if (this.grants.clock === false) throw new AutomationError('grant.denied', 'moving the clock needs the `clock` grant', 'clock.advance');
  }

  /**
   * `app.reset`: the app the host was started with, rebuilt in a fresh host on a fresh virtual
   * clock (seeded with `seed`, else the launch seed), then settled. The runner calls it between
   * tests, so a worker keeps one process.
   */
  async reset({ seed, timeout } = {}) {
    const r = await this.call('app.reset', { seed, timeout });
    if (r?.seed !== undefined && r.seed !== null) process.env.SVELTERS_SEED = String(r.seed);
    return r;
  }

  /**
   * `app.mount` (E2E-INSPECTOR §2.4): `component` (a `.svelte` path, or a `file:` URL, inside an
   * app: its package.json gives the toolkit) alone in a fresh host on a fresh virtual clock,
   * compiled live, with `props`, in a `size` window. Returns the mounted component as a locator
   * over the host, with `setProps`.
   */
  async mount(component, { export: exportName, props, size, seed, timeout } = {}) {
    const module = component instanceof URL ? fileURLToPath(component) : resolvePath(String(component));
    const r = await this.call('app.mount', { module, export: exportName, props: props ?? {}, size, seed, timeout });
    if (r?.seed !== undefined && r.seed !== null) process.env.SVELTERS_SEED = String(r.seed);
    return new Mounted(this, module);
  }

  call(method, params) {
    return this.client.call(method, params);
  }

  /** Scopes to the root with this key (`root=key`). */
  root(key) {
    return new Locator(this, `root=${key}`);
  }
  waitSettled({ timeout } = {}) {
    return this.call('wait.settled', { timeout });
  }
  async snapshot() {
    return (await this.call('scene.snapshot', {})).text;
  }

  /** `app.health`, answered by the app's IO thread on a second connection (works while hung). */
  async health() {
    return this.#side((c) => c.call('app.health'));
  }
  /** `app.interrupt`: breaks a runaway JS loop (second connection, so it never queues). */
  async interrupt() {
    return this.#side((c) => c.call('app.interrupt'));
  }
  async #side(f) {
    const c = await Client.connect(this.endpoint);
    try {
      return await f(c);
    } finally {
      c.close();
    }
  }

  /** Asks the app to exit, then makes sure it did. */
  async close() {
    if (this.child && this.child.exitCode === null) {
      const exited = new Promise((r) => this.child.once('exit', r));
      await this.call('app.exit').catch(() => {});
      const t = setTimeout(() => this.child.kill('SIGKILL'), 5_000);
      await exited;
      clearTimeout(t);
    }
    this.client.close();
  }
}

/** A component from `app.mount`: locators over its host, and its props. */
export class Mounted extends Locator {
  constructor(app, module) {
    super(app, '');
    this.module = module;
  }
  /** Shallow-merges `props` into the component's props (`app.set_props`); its state is kept. */
  setProps(props, { timeout } = {}) {
    return this.app.call('app.set_props', { props, timeout });
  }
}

/** Connects to an app that already listens on `endpoint`. */
export async function attach(endpoint, { timeout } = {}) {
  const client = await Client.connect(endpoint, { timeout });
  const hello = await client.call('session.hello', { client: '@svelters/test' });
  return new App(client, { endpoint, hello });
}

/**
 * Spawns `bin [app] --automation <endpoint> --headless --clock virtual [--seed N] [--step-ms MS]`
 * (the reference host `svelters-run`, or an app binary that takes the same flags), waits for its
 * `SVELTERS_AUTOMATION=<endpoint>` line and says hello. `endpoint` defaults to `tcp:0`.
 */
export async function launch({ bin, app, args = [], clock = 'virtual', endpoint = 'tcp:0', seed, stepMs, headless = true, timeout = 60_000, env, stderr = 'inherit' } = {}) {
  const argv = [...(app ? [app] : []), ...args, '--automation', endpoint];
  if (headless) argv.push('--headless', '--clock', clock);
  if (seed !== undefined) argv.push('--seed', String(seed));
  if (stepMs !== undefined) argv.push('--step-ms', String(stepMs));
  const child = spawn(bin, argv, { stdio: ['ignore', 'pipe', stderr], env: env ?? process.env });
  // `stderr: 'pipe'` (the runner): the last lines are kept for a failure report.
  child.stderrTail = [];
  if (child.stderr) {
    createInterface({ input: child.stderr }).on('line', (l) => {
      child.stderrTail.push(l);
      if (child.stderrTail.length > 40) child.stderrTail.shift();
    });
  }
  const listening = await new Promise((resolve, reject) => {
    const t = setTimeout(() => {
      child.kill('SIGKILL');
      reject(new AutomationError('timeout', `${bin} did not start listening in ${timeout} ms`));
    }, timeout);
    const lines = createInterface({ input: child.stdout });
    lines.on('line', (line) => {
      const m = /^SVELTERS_AUTOMATION=(.+)$/.exec(line.trim());
      if (m) {
        clearTimeout(t);
        resolve(m[1]);
      }
    });
    child.once('exit', (code) => {
      clearTimeout(t);
      // Give the stderr reader a moment to collect the reason.
      setTimeout(() => {
        const why = child.stderrTail.length ? `:\n${child.stderrTail.join('\n')}` : '';
        reject(new AutomationError('transport.closed', `${bin} exited (${code}) before listening${why}`));
      }, 20);
    });
    child.once('error', (e) => {
      clearTimeout(t);
      reject(new AutomationError('transport.io', `${bin}: ${e.message}`));
    });
  });
  const client = await Client.connect(listening, { timeout });
  const hello = await client.call('session.hello', { client: '@svelters/test' });
  if (seed !== undefined) process.env.SVELTERS_SEED = String(seed);
  return new App(client, { child, endpoint: listening, hello });
}

/** Strips the common indentation and blank edge lines (template literals in tests). */
export function dedent(text) {
  const lines = text.replace(/\t/g, '  ').split('\n');
  while (lines.length && !lines[0].trim()) lines.shift();
  while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
  const indent = Math.min(...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length));
  return lines.map((l) => l.slice(indent).trimEnd()).join('\n');
}

/**
 * Web-first assertions. Each sends one `wait.for` that the app evaluates after every settle until
 * it holds or the timeout (virtual ms) passes; a failure rejects with the app's reason.
 */
export function expect(locator) {
  const waitFor = (state, exp, { timeout } = {}) =>
    locator.app.call('wait.for', { selector: locator.selector, state, expect: exp, timeout });
  return {
    toBeVisible: (o) => waitFor('visible', undefined, o),
    toBeHidden: (o) => waitFor('hidden', undefined, o),
    toBeAttached: (o) => waitFor('attached', undefined, o),
    toBeDetached: (o) => waitFor('detached', undefined, o),
    toBeEnabled: (o) => waitFor('enabled', undefined, o),
    toHaveText: (text, o = {}) => waitFor('attached', { text, exact: o.exact ?? true }, o),
    toContainText: (text, o) => waitFor('attached', { text, exact: false }, o),
    toHaveValue: (value, o = {}) => waitFor('attached', { value: String(value), exact: o.exact ?? true }, o),
    toHaveCount: (count, o) => waitFor('attached', { count }, o),
    /**
     * The settled scene snapshot of the locator (the whole app for `app`) equals `expected`
     * (indentation-insensitive). The YAML snapshot is the default regression artefact (§1.8).
     */
    async toMatchSceneSnapshot(expected, o = {}) {
      await locator.app.call('wait.settled', { timeout: o.timeout });
      const params = locator.selector ? { selector: locator.selector } : {};
      const actual = dedent((await locator.app.call('scene.snapshot', params)).text);
      const want = dedent(expected);
      if (actual !== want) {
        const err = new AutomationError('snapshot.mismatch', `scene snapshot differs\n--- expected\n${want}\n--- actual\n${actual}`);
        err.expected = want;
        err.actual = actual;
        throw err;
      }
    },
  };
}

export { test } from './runner/api.js';
