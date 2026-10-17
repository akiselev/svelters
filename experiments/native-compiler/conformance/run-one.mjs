// Runs one prepared sample (out/<name>: the sample's files + _compiled/*.svelte.js) the way
// Svelte's tests/runtime-legacy/shared.ts runs the `dom` variant of a runes test, on the DOM host
// instead of Svelte's DOM runtime. Prints one JSON line: { outcome, message }.
//   outcome: pass | mismatch (an assertion failed) | runtime_error (anything else threw)
// Started by run.mjs as `node --conditions=browser --conditions=development --import ./register.mjs run-one.mjs <dir>`.
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { setImmediate } from 'node:timers/promises';
import { JSDOM, VirtualConsole } from 'jsdom';

const dir = path.resolve(process.argv[2]);

// --- a jsdom window as vitest's jsdom environment provides it (globals copied onto globalThis)
const virtualConsole = new VirtualConsole();
virtualConsole.on('log', (...a) => console.log(...a));
virtualConsole.on('warn', (...a) => console.warn(...a));
virtualConsole.on('error', (...a) => console.error(...a));
const dom = new JSDOM('<!doctype html><html><head></head><body></body></html>', { pretendToBeVisual: true, virtualConsole, url: 'http://localhost/' });
const { window } = dom;
// Node has its own versions of these; the DOM's must win (jsdom rejects a Node `Event`), as in
// vitest's jsdom environment.
const OVERRIDE = new Set(['Event', 'EventTarget', 'CustomEvent', 'MessageEvent', 'ErrorEvent', 'AbortController', 'AbortSignal', 'DOMException', 'Blob', 'File', 'FormData', 'Headers', 'URL', 'URLSearchParams', 'navigator']);
for (const key of Object.getOwnPropertyNames(window)) {
  if (key in globalThis && !OVERRIDE.has(key)) continue;
  try {
    globalThis[key] = window[key];
  } catch {}
}
globalThis.window = window;
globalThis.document = window.document;
globalThis.requestAnimationFrame = globalThis.setTimeout;
window.matchMedia = (media) => ({ matches: false, media, addEventListener() {}, removeEventListener() {} });

const { assert } = await import('chai');
const { assert_html_equal, assert_html_equal_with_options } = await import('./shims/html-equal.mjs');
const { DomHost } = await import('./dom-host.mjs');
const { flushSync } = await import('svelte');
const $ = await import('svelte/internal/client');
const $n = await import('@svelters/native-runtime');

const result = (outcome, err) => {
  const fmt = (v) => {
    try {
      return typeof v === 'string' ? JSON.stringify(v) : JSON.stringify(v, (_, x) => (typeof x === 'function' ? `[fn ${x.name}]` : x));
    } catch {
      return String(v);
    }
  };
  let message = err == null ? undefined : String(err?.message ?? err);
  if (err?.name === 'AssertionError' && ('actual' in err || 'expected' in err)) message += `\n  actual:   ${fmt(err.actual)}\n  expected: ${fmt(err.expected)}`;
  message = message?.slice(0, 3000);
  process.stdout.write('\n@@RESULT ' + JSON.stringify({ outcome, message, name: err?.name }) + '\n');
};

const config = (await import(pathToFileURL(path.join(dir, '_config.js')).href)).default;

let unhandled = null;
process.on('unhandledRejection', (e) => (unhandled ??= e));

const logs = [];
const warnings = [];
const errors = [];
const console_log = console.log;
const console_warn = console.warn;
const console_error = console.error;
let intercept_errors = false;
let unintended_error = false;

// shared.ts's "crude static analysis" of which test() parameters are used
{
  const str = config.test?.toString() ?? '';
  let n = 0;
  let i = 0;
  while (i < str.length) {
    if (str[i] === '(') n++;
    if (str[i] === ')' && --n === 0) break;
    i++;
  }
  const params = str.slice(0, i);
  if (params.includes('logs')) console.log = (...args) => void logs.push(...args);
  if (params.includes('warnings') || config.warnings) {
    console.warn = (...args) => {
      if (typeof args[0] === 'string' && args[0].startsWith('%c[svelte]')) {
        let message = args[0];
        message = message.slice(message.indexOf('%c', 2) + 2);
        const lines = message.split('\n');
        if (lines.at(-1)?.startsWith('https://svelte.dev/e/')) lines.pop();
        warnings.push(lines.join('\n'));
      } else warnings.push(...args);
    };
  }
  if (params.includes('errors') || config.errors) {
    intercept_errors = true;
    console.error = (...args) => void errors.push(...args);
  }
}

window.addEventListener(
  'error',
  (e) => {
    if (intercept_errors) {
      const detail = e.error;
      const is_error = detail && detail.name && detail.message !== undefined && detail.stack;
      errors.push(`Error: Uncaught ${is_error ? `[${detail.name}: ${detail.message}]` : String(detail)}\n${detail?.stack ?? ''}`, detail);
    } else unhandled ??= e.error;
    e.preventDefault();
  },
  true,
);

const restore = () => {
  console.log = console_log;
  console.warn = console_warn;
  console.error = console_error;
};

let outcome = 'pass';
let failure = null;
try {
  window.document.body.innerHTML = '<main></main>';
  const target = window.document.querySelector('main');
  const host = new DomHost(window.document);
  globalThis.__svelters_host = host;

  const mod = await import(pathToFileURL(path.join(dir, 'main.svelte')).href);
  logs.length = warnings.length = 0;
  config.before_test?.();

  const props = $.proxy({ ...(config.props || {}) });
  const unmount = $n.mount(mod.default, { props, target: host.adopt(target), transformError: config.transformError });

  if (config.error) {
    unintended_error = true;
    assert.fail('Expected a runtime error');
  }
  if (config.html) {
    flushSync();
    assert_html_equal_with_options(target.innerHTML, config.html, {
      preserveComments: config.withoutNormalizeHtml === 'only-strip-comments' ? false : undefined,
      withoutNormalizeHtml: !!config.withoutNormalizeHtml,
    });
  }
  try {
    if (config.test) {
      if (!config.skip_initial_flushSync) flushSync();
      await config.test({
        assert: { ...assert, htmlEqual: assert_html_equal, htmlEqualWithOptions: assert_html_equal_with_options },
        variant: 'dom',
        component: props,
        instance: unmount.exports,
        mod,
        ok: (v) => {
          if (!v) throw new Error(`Expected truthy value, got ${v}`);
        },
        target,
        snapshot: undefined,
        window,
        raf: { tick: () => flushSync() },
        compileOptions: {},
        logs,
        warnings,
        errors,
        hydrate: () => {
          throw new Error('Ensure dom mode is skipped');
        },
      });
      flushSync();
    }
    if (config.runtime_error && !unhandled) {
      unintended_error = true;
      assert.fail('Expected a runtime error');
    }
  } finally {
    unmount();
    if (config.warnings) assert.deepEqual(warnings, config.warnings);
    else if (warnings.length && console.warn === console_warn) {
      unintended_error = true;
      assert.fail(`Received unexpected warnings: ${warnings.join(' | ').slice(0, 500)}`);
    }
    if (config.errors) assert.deepEqual(errors, config.errors);
    else if (errors.length && console.error === console_error) {
      unintended_error = true;
      assert.fail(`Received unexpected errors: ${errors.map(String).join(' | ').slice(0, 500)}`);
    }
    assert_html_equal(target.innerHTML, '', 'Expected component to unmount and leave nothing behind after it was destroyed');
    if (unhandled && !config.expect_unhandled_rejections) throw unhandled;
  }
} catch (err) {
  try {
    if (config.runtime_error) assert.include(err.message, config.runtime_error);
    else if (config.error && !unintended_error) assert.include(err.message, config.error);
    else throw err;
  } catch (e) {
    failure = e;
    outcome = e?.name === 'AssertionError' ? 'mismatch' : 'runtime_error';
    if (process.env.STACK) console.error(e?.stack);
  }
} finally {
  config.after_test?.();
  await setImmediate();
  restore();
}
result(outcome, failure);
process.exit(0);
