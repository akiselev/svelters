// tools/oplog-compare.mjs: state comparison of op logs.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compare } from '../../../tools/oplog-compare.mjs';

const base = `create window #1
create label #2
set #2 label "a"
insert #1 #2 before=end slot=default
create button #3
listen #3 clicked on
insert #1 #3 before=end slot=default
call #1 present []
event #3 clicked []
`;

test('reordered ops with the same states are equal', () => {
  const b = base
    .replace('set #2 label "a"\ninsert #1 #2 before=end slot=default\n', 'insert #1 #2 before=end slot=default\nset #2 label "a"\n')
    .concat('remove #1 #2\ndestroy #2\n');
  const a = base.concat('remove #1 #2\ndestroy #2\n');
  assert.equal(compare(a, b).equal, true);
});

test('a differing prop, child order or listened signal is reported at its boundary', () => {
  assert.equal(compare(base, base.replace('"a"', '"b"')).equal, false);
  const swapped = base.replace('insert #1 #3 before=end', 'insert #1 #3 before=#2');
  assert.match(compare(base, swapped).problems[0], /before "event #3 clicked \[\]"/);
  assert.equal(compare(base, base.replace('listen #3 clicked on\n', '')).equal, false);
});

test('sync / deferred lines are boundaries and sync results are part of the state', () => {
  const s = base.concat('sync #1 close-request []\nresult #1 true\ndeferred #3 clicked []\n');
  assert.equal(compare(s, s).equal, true);
  assert.equal(compare(s, s.replace('result #1 true', 'result #1 null')).equal, false);
  assert.equal(compare(s, s.replace('deferred #3', 'sync #3')).equal, false);
});

test('defaults: a prop written with its default value counts as unset', () => {
  const extra = base.replace('create button #3\n', 'create button #3\nset #3 label ""\n');
  assert.equal(compare(base, extra).equal, false);
  assert.equal(compare(base, extra, { defaults: { label: '' } }).equal, true);
});
