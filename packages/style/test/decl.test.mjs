// The decl dialect (ST2): AppKit stylesheets as structured rules, and the runtime matching half.
import assert from 'node:assert/strict';
import test from 'node:test';
import { parseCss } from 'svelte/compiler';
import { declValueProblem, expandDecl, selectorMatches, translateDeclSheet, byCascade } from '../src/decl.js';
import { translateSheet } from '../src/translate.js';

const tr = (css, tags = null) => translateDeclSheet(css, 'appkit', { parseCss, tags });
const el = (tag, classes = [], id = null) => ({ tag, classes, id });

test('rules: selectors, states, dark, specificity and declarations with var() kept', () => {
  const { rules, diagnostics } = tr(`
    .card { background-color: var(--sv-surface); border-radius: 10px; }
    .card:hover { background-color: #eee; }
    stack-view > .title { color: red; font-size: 18px; }
    @media (prefers-color-scheme: dark) { .card { border-color: #444; } }
  `);
  assert.deepEqual(diagnostics, []);
  assert.equal(rules.length, 4);
  assert.deepEqual(rules[0].sel, [{ tag: null, classes: ['card'], id: null, comb: null }]);
  assert.deepEqual(rules[0].decls, [['background-color', 'var(--sv-surface)'], ['border-radius', '10px']]);
  assert.equal(rules[1].state, 'hover');
  assert.deepEqual(rules[1].spec, [0, 2, 0]);
  assert.deepEqual(rules[2].sel[1], { tag: null, classes: ['title'], id: null, comb: '>' });
  assert.deepEqual(rules[2].spec, [0, 1, 1]);
  assert.equal(rules[3].dark, true);
});

test('setter properties are left to the compiler; :where() is flattened; focus-visible is focus', () => {
  const { rules } = tr(`.row:where(.svelte-x) { gap: 8px; color: blue; } .f:focus-visible { color: red; }`);
  assert.deepEqual(rules[0].decls, [['color', 'blue']]);
  assert.deepEqual(rules[0].sel[0].classes, ['row', 'svelte-x']);
  assert.equal(rules[1].state, 'focus');
});

test('diagnostics name the toolkit: unknown properties, bad values, unsupported selectors', () => {
  const { rules, diagnostics } = tr(
    `.a { colour: red; color: 12px; }
     .b + .c { color: red; }
     .d:hover .e { color: red; }
     .f::before { color: red; }
     @media (min-width: 400px) { .g { color: red; } }
     nope { color: red; }`,
    ['button'],
  );
  const text = diagnostics.map((d) => `${d.level} [${d.toolkit}] ${d.message}`).join('\n');
  assert.match(text, /warning \[appkit\] property 'colour' is not an appkit style property.*did you mean 'color'/);
  assert.match(text, /error \[appkit\] '12px' is not a colour for color/);
  assert.match(text, /combinator '\+' is not supported/);
  assert.match(text, /:hover is only supported on the last compound/);
  assert.match(text, /::before is not supported/);
  assert.match(text, /@media \(min-width: 400px\) is not supported/);
  assert.match(text, /<nope> is not an element on appkit/);
  assert.equal(rules.length, 0);
});

test('values: colours, lengths, enums, border and shadow grammars', () => {
  assert.equal(declValueProblem('color', 'color', 'rgba(0, 0, 0, 0.5)', []), null);
  assert.equal(declValueProblem('color', 'color', '#abc', []), null);
  assert.equal(declValueProblem('color', 'color', 'X', []), null); // a var()
  assert.ok(declValueProblem('color', 'color', 'reddish', []));
  assert.equal(declValueProblem('width', 'length', '12pt', []), null);
  assert.ok(declValueProblem('width', 'length', '50%', []));
  assert.equal(declValueProblem('cursor', 'enum', 'pointer', ['default', 'pointer']), null);
  assert.equal(declValueProblem('border', 'border', '1px solid #ccc', []), null);
  assert.ok(declValueProblem('border', 'border', '1px dashed #ccc', []));
  assert.equal(declValueProblem('box-shadow', 'shadow', '0 2px 8px rgba(0,0,0,0.2)', []), null);
  assert.ok(declValueProblem('box-shadow', 'shadow', 'inset 0 1px 2px black', []));
});

test('translateSheet hands decl toolkits to the decl translator', () => {
  const r = translateSheet('.x { color: red; }', 'appkit');
  assert.equal(r.css, '');
  assert.equal(r.declRules.length, 1);
});

test('matching: descendant and child combinators along the ancestor chain', () => {
  const chain = [el('window'), el('stack-view', ['app']), el('stack-view', ['row', 'done']), el('text-field', ['label'], 'main')];
  const sel = (s) => tr(`${s} { color: red; }`).rules[0].sel;
  assert.ok(selectorMatches(sel('.label'), chain));
  assert.ok(selectorMatches(sel('.row.done .label'), chain));
  assert.ok(selectorMatches(sel('.app .label'), chain));
  assert.ok(selectorMatches(sel('.row > .label'), chain));
  assert.ok(!selectorMatches(sel('.app > .label'), chain));
  assert.ok(selectorMatches(sel('text-field#main'), chain));
  assert.ok(!selectorMatches(sel('.row.active .label'), chain));
  assert.ok(selectorMatches(sel('.app .row .label'), chain));
  assert.ok(!selectorMatches(sel('.row .app .label'), chain));
});

test('cascade order: specificity, then source order', () => {
  const { rules } = tr(`.a.b { color: red; } #i { color: blue; } .a { color: green; } button { color: gray; }`);
  const order = [...rules].sort(byCascade).map((r) => r.decls[0][1]);
  assert.deepEqual(order, ['gray', 'green', 'red', 'blue']);
});

test('shorthands expand to what the backend applies', () => {
  assert.deepEqual(expandDecl('border', '1px solid #ccc'), [['border-width', '1px'], ['border-color', '#ccc']]);
  assert.deepEqual(expandDecl('border', 'none'), [['border-width', '0']]);
  assert.deepEqual(expandDecl('background', 'red'), [['background-color', 'red']]);
  assert.deepEqual(expandDecl('border-style', 'solid'), []);
  assert.deepEqual(expandDecl('color', 'red'), [['color', 'red']]);
});
