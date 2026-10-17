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
 * { name, source, script, sources?, scripts?, jsExpected?, handwritten?, own, targets? }
 *   handwritten: the hand-written equivalent's op log (the intended code shape), or null for a
 *     JS-pipeline comparison only (state-equal with jsExpected at every boundary; identical op
 *     logs are reported as such);
 *   rejectOn: { target: code } expected compile-time rejections;
 *   targets: compile targets (default: the real backends);
 *   sources / scripts: { target: path } overriding source / script for one target.
 *
 * The shared fixtures are written in the GTK vocabulary, so they compile for gtk (and testhost);
 * the qt target compiles their Qt twins (fixtures/<name>-qt, cases named `<name>-qt`) instead. An own
 * fixture (../fixtures/<name>) is compiled for qt from its Qt-vocabulary copy <name>/qt/ when it has
 * one (App.svelte and its components; qt/script.json when the handles differ).
 */
export function cases() {
  const out = [];
  const fixture = (name) => path.join(repo, 'fixtures', name);
  // further scripts run live on both pipelines' bundles (JS: <fixture>/dist/app.js)
  const extraScripts = (dir) => (fs.existsSync(path.join(dir, 'script.deferred.json')) ? [path.join(dir, 'script.deferred.json')] : []);
  /** The Qt twin of a shared fixture: a qt-only case compared with the twin's JS pipeline. */
  const qtTwin = (name, extra = {}) => {
    const dir = fixture(`${name}-qt`);
    if (!fs.existsSync(dir)) return;
    out.push({
      name: `${name}-qt`,
      source: path.join(dir, 'src/App.svelte'),
      script: path.join(dir, 'script.json'),
      jsExpected: path.join(dir, 'expected.oplog'),
      handwritten: null,
      extraScripts: extraScripts(dir),
      targets: ['qt'],
      ...extra,
    });
  };
  // virtual-list / lists-stress: `<ListView>` from @svelters/lists (virtualized native list); the
  // native pipeline resolves it to @svelters/native-runtime/lists. No hand-written equivalent.
  const lists = ['virtual-list', 'lists-stress', 'lists-recycle'];
  for (const name of ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers', ...lists]) {
    const nativeScript = fs.existsSync(path.join(here, 'scripts', `${name}.native.json`)) ? path.join(here, 'scripts', `${name}.native.json`) : undefined;
    out.push({
      name,
      source: path.join(fixture(name), 'src/App.svelte'),
      script: path.join(fixture(name), 'script.json'),
      jsExpected: path.join(fixture(name), 'expected.oplog'),
      handwritten: lists.includes(name) ? null : path.join(here, '../handwritten', name, 'expected.oplog'),
      // The hand-written equivalent predates master's bind:this version of this fixture: only
      // the JS-pipeline state comparison applies.
      handwrittenStale: name === 'native-close',
      nativeScript,
      extraScripts: extraScripts(fixture(name)),
      targets: ['gtk'],
    });
    qtTwin(name, { nativeScript });
  }
  // X2/X3 fixtures of the JS pipeline (child attributes, listen phases, sync handlers) and the
  // bind:this ones: compared with the JS pipeline only (no hand-written equivalents). <entry>
  // has no `key-pressed` on GTK (a controller signal): those two run on the testhost target only
  // and are rejected on the real spec.
  const jsOnly = {
    'child-attrs': {},
    'sync-handlers': { gtk: 'unknown_event' },
    'listen-phases': { gtk: 'unknown_event' },
    'bind-this': {},
    'borrowed-walk': {},
  };
  for (const [name, rejectOn] of Object.entries(jsOnly)) {
    out.push({
      name,
      source: path.join(fixture(name), 'src/App.svelte'),
      script: path.join(fixture(name), 'script.json'),
      jsExpected: path.join(fixture(name), 'expected.oplog'),
      handwritten: null,
      rejectOn,
      targets: ['gtk', 'testhost'],
    });
    qtTwin(name);
  }
  const own = path.join(here, '../fixtures');
  if (fs.existsSync(own)) {
    for (const name of fs.readdirSync(own).sort()) {
      const dir = path.join(own, name);
      if (!fs.existsSync(path.join(dir, 'App.svelte'))) continue;
      const c = { name, source: path.join(dir, 'App.svelte'), script: path.join(dir, 'script.json'), dir, own: true, sources: {}, scripts: {} };
      for (const t of BACKENDS) {
        if (fs.existsSync(path.join(dir, t, 'App.svelte'))) c.sources[t] = path.join(dir, t, 'App.svelte');
        if (fs.existsSync(path.join(dir, t, 'script.json'))) c.scripts[t] = path.join(dir, t, 'script.json');
      }
      out.push(c);
    }
  }
  return out;
}

/** The component a case compiles for `target`. */
export const sourceOf = (c, target) => c.sources?.[target] ?? c.source;
/** The script a case runs with on `target`. */
export const scriptOf = (c, target) => c.scripts?.[target] ?? c.script;

/** The targets a case is compiled for. */
export function targetsOf(c) {
  const names = c.targets ?? BACKENDS;
  return TARGETS.filter((t) => names.includes(t.name));
}
