// The compiled cases: the existing JS-pipeline fixtures (same App.svelte, same script) and the
// NC1-only fixtures under ../fixtures (native binding, styling, slots, teardown).
import path from 'node:path';
import fs from 'node:fs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
/** The real backends (native.mjs runs these on the toolkits). */
export const BACKENDS = ['gtk', 'qt'];

/**
 * Compile targets: the real backends, plus `testhost` = the GTK spec with the vocabulary the
 * JS-pipeline fixtures assume on the Node TestHost (testhost-overlay.json, merged by build.mjs into
 * out/testhost.spec.json). Only cases that list it in `targets` are compiled for it.
 */
export const TARGETS = [
  { name: 'gtk', backend: 'gtk' },
  { name: 'qt', backend: 'qt' },
  { name: 'testhost', backend: 'gtk', spec: path.join(here, 'out/testhost.spec.json'), overlay: path.join(here, 'testhost-overlay.json') },
];

/**
 * { name, source, script, jsExpected?, handwritten?, own, targets? }
 *   handwritten: the hand-written equivalent's op log (the intended code shape), or null for a
 *     JS-pipeline comparison only (state-equal with jsExpected at every boundary; identical op
 *     logs are reported as such);
 *   rejectOn: { target: code } expected compile-time rejections;
 *   targets: compile targets (default: the real backends).
 */
export function cases() {
  const out = [];
  // virtual-list / lists-stress: `<ListView>` from @svelters/lists (virtualized native list); the
  // native pipeline resolves it to @svelters/native-runtime/lists. No hand-written equivalent.
  const lists = ['virtual-list', 'lists-stress', 'lists-recycle'];
  for (const name of ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers', ...lists]) {
    out.push({
      name,
      source: path.join(repo, 'fixtures', name, 'src/App.svelte'),
      script: path.join(repo, 'fixtures', name, 'script.json'),
      jsExpected: path.join(repo, 'fixtures', name, 'expected.oplog'),
      handwritten: lists.includes(name) ? null : path.join(here, '../handwritten', name, 'expected.oplog'),
      // Rejected at compile time by the backend spec (the JS pipeline fails at runtime instead).
      rejectOn: name === 'if-toggle' ? { qt: 'unknown_element' } : {},
      // The hand-written equivalent predates master's bind:this version of this fixture: only
      // the JS-pipeline state comparison applies.
      handwrittenStale: name === 'native-close',
      nativeScript: fs.existsSync(path.join(here, 'scripts', `${name}.native.json`)) ? path.join(here, 'scripts', `${name}.native.json`) : undefined,
      // further scripts run live on both pipelines' bundles (JS: fixtures/<name>/dist/app.js)
      extraScripts: fs.existsSync(path.join(repo, 'fixtures', name, 'script.deferred.json')) ? [path.join(repo, 'fixtures', name, 'script.deferred.json')] : [],
    });
  }
  // X2/X3 fixtures of the JS pipeline (child attributes, listen phases, sync handlers) and the
  // bind:this ones: compared with the JS pipeline only (no hand-written equivalents). <entry>
  // has no `key-pressed` on GTK (a controller signal) nor on Qt (`key-press`): those two run on
  // the testhost target only and are rejected on the real specs.
  const jsOnly = {
    'child-attrs': {},
    'sync-handlers': { gtk: 'unknown_event', qt: 'unknown_event' },
    'listen-phases': { gtk: 'unknown_event', qt: 'unknown_event' },
    'bind-this': {},
    'borrowed-walk': {},
  };
  for (const [name, rejectOn] of Object.entries(jsOnly)) {
    out.push({
      name,
      source: path.join(repo, 'fixtures', name, 'src/App.svelte'),
      script: path.join(repo, 'fixtures', name, 'script.json'),
      jsExpected: path.join(repo, 'fixtures', name, 'expected.oplog'),
      handwritten: null,
      rejectOn,
      targets: ['gtk', 'qt', 'testhost'],
    });
  }
  const own = path.join(here, '../fixtures');
  if (fs.existsSync(own)) {
    for (const name of fs.readdirSync(own).sort()) {
      const dir = path.join(own, name);
      if (!fs.existsSync(path.join(dir, 'App.svelte'))) continue;
      out.push({ name, source: path.join(dir, 'App.svelte'), script: path.join(dir, 'script.json'), dir, own: true });
    }
  }
  return out;
}

/** The targets a case is compiled for. */
export function targetsOf(c) {
  const names = c.targets ?? BACKENDS;
  return TARGETS.filter((t) => names.includes(t.name));
}
