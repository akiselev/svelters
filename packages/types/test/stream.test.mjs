// E3: NativeStream and persist against a fake host that plays the prelude's part (binds the stream port,
// hands items over, settles the call). The real path is covered by crates/svelters/tests/reactive.rs.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { NativeObject, persist, unpersist, registerClass, streamStatic } from '../src/core.js';

const STREAM = Symbol.for('svelters.stream');

/** A fake host whose `callStatic` starts a stream call: returns its Promise and the port controls. */
function streamHost() {
  const calls = [];
  globalThis.__svelters_host = {
    callStatic(kind, method, args) {
      const opts = args[args.length - 1];
      const port = opts[STREAM];
      const call = { kind, method, args: args.slice(0, -1), acked: 0, closed: false, port };
      call.promise = new Promise((res, rej) => {
        call.resolve = res;
        call.reject = rej;
      });
      port.bind(
        (n) => (call.acked += n),
        () => {
          call.closed = true;
          call.resolve(undefined);
        },
      );
      calls.push(call);
      return call.promise;
    },
  };
  return calls;
}

test('a stream yields its items in order, acks what it hands out, then ends', async () => {
  const calls = streamHost();
  const s = streamStatic('app:m', 'count', [3], undefined, (x) => x * 10);
  const c = calls[0];
  assert.deepEqual(c.args, [3]);
  c.port.items([1, 2]);
  c.port.items([3]);
  c.resolve(null);
  const got = [];
  for await (const x of s) got.push(x);
  assert.deepEqual(got, [10, 20, 30]);
  assert.equal(c.acked, 3);
  assert.equal(c.closed, false);
});

test('leaving the loop early closes the call; an error comes after the items', async () => {
  const calls = streamHost();
  const s = streamStatic('app:m', 'count', [], {});
  calls[0].port.items([1, 2, 3, 4]);
  for await (const x of s) if (x === 2) break;
  assert.equal(calls[0].closed, true);

  const t = streamStatic('app:m', 'fail', [], {});
  calls[1].port.items(['a']);
  calls[1].reject(Object.assign(new Error('boom'), { code: 'boom' }));
  const got = [];
  await assert.rejects(async () => {
    for await (const x of t) got.push(x);
  }, /boom/);
  assert.deepEqual(got, ['a']);
});

test('batches() yields what arrived together and acks it', async () => {
  const calls = streamHost();
  const s = streamStatic('app:m', 'count', [], {});
  calls[0].port.items([1, 2, 3]);
  const it = s.batches();
  assert.deepEqual((await it.next()).value, [1, 2, 3]);
  assert.equal(calls[0].acked, 3);
  calls[0].port.items([4]);
  calls[0].resolve(null);
  assert.deepEqual((await it.next()).value, [4]);
  assert.equal((await it.next()).done, true);
});

class Thing extends NativeObject {
  static native = 'app:m.Thing';
  static [Symbol.for('svelters.types.construct')]() {
    return 7;
  }
}

test('persist makes once, then hands back the persisted object as its class', () => {
  const kept = new Map();
  let made = 0;
  globalThis.__svelters_host = {
    persist(key, value) {
      if (value === undefined) return kept.has(key) ? [kept.get(key), 'app:m.Thing'] : null;
      if (value === null) kept.delete(key);
      else kept.set(key, value.svlHandle);
      return true;
    },
  };
  registerClass(Thing);
  const a = persist('k', () => (made++, new Thing()));
  assert.equal(made, 1);
  assert.equal(kept.get('k'), 7);
  const b = persist('k', () => (made++, new Thing()));
  assert.equal(made, 1, 'the second runtime does not make another');
  assert.equal(b, a, 'the same wrapper while it lives');
  unpersist('k');
  assert.equal(kept.has('k'), false);
  // A host without persist (the Node TestHost): just make().
  globalThis.__svelters_host = {};
  persist('k', () => (made++, new Thing()));
  assert.equal(made, 2);
});
