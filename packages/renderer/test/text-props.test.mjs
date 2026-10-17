// Text children fold into the prop the generated TEXT_PROPS names (metadata/semantics/<toolkit>.json text_content).
import test from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root, TEXT_PROPS, LABEL_KINDS } = await import('../src/index.js');

const take = () => th.lines.splice(0).join('\n');

test('TEXT_PROPS is generated: button has a candidate per toolkit, togglebutton inherits the GTK prop', () => {
  assert.deepEqual(TEXT_PROPS.button.slice(0, 2), ['label', 'title']);
  assert.deepEqual(TEXT_PROPS.togglebutton, ['label']);
  assert.deepEqual([...LABEL_KINDS].sort(), Object.keys(TEXT_PROPS).sort());
});

for (const tag of ['button', 'togglebutton']) {
  test(`<${tag}> folds a text child and an {expr} text child into its text prop`, () => {
    const win = r.createElement('window');
    const e = r.createElement(tag);
    const text = r.createTextNode('Hi');
    r.insert(e, text, null);
    r.insert(win, e, null);
    r.insert(root, win, null);
    assert.match(take(), new RegExp(`create ${tag} #(\\d+)\\nset #\\1 label "Hi"`));
    r.setText(text, 'Yo'); // {expr} updates
    assert.equal(take(), `set #${e.handle} label "Yo"`);
    const more = r.createTextNode('!');
    r.insert(e, more, null);
    assert.equal(take(), `set #${e.handle} label "Yo!"`);
  });
}
