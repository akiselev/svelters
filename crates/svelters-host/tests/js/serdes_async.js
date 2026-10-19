// Async native methods: Promise results, rejections as NativeError, AbortSignal and late completions.
// Runs inside an event so the backend has its host callbacks; reactions run after it, at the checkpoint.
const h = globalThis.__svelters_host;
const t = h.create('thing');
h.listen(t, 'changed', true);
const log = (...a) => console.log(a.join(' '));
const err = (e) => [e.name, e.domain, e.code, e.message].join(' ');

globalThis.__svelters_dispatch = () => {
  // resolve: the Promise is pending until the backend completes; reactions run later
  const p1 = h.call(t, 'slow', []);
  p1.then((v) => log('resolved', v));
  h.call(t, 'complete', ['payload']);

  // reject with a NativeError (domain, code name, message)
  const p2 = h.call(t, 'slow', []);
  p2.catch((e) => log('rejected', err(e)));
  h.call(t, 'completeError', []);

  // arguments before the completion callback
  h.call(t, 'slowOf', ['label']).then((v) => log('slowOf resolved', v));
  h.call(t, 'complete', [42]);

  // a backend that cannot start the operation rejects the Promise instead of throwing
  h.call(t, 'slowFail', []).catch((e) => log('start failure rejected:', e.domain, e.message));

  // AbortSignal -> backend->cancel -> the backend completes with its cancellation error
  const ac = new AbortController();
  h.call(t, 'slow', [{ signal: ac.signal }]).catch((e) => log('aborted', err(e)));
  ac.abort();
  // an already aborted signal cancels right after the start
  const done = new AbortController();
  done.abort();
  h.call(t, 'slow', [{ signal: done.signal }]).catch((e) => log('pre-aborted', err(e)));

  // completing after the owner died is an error for the backend, not a crash
  const owner = h.create('thing');
  // ... and the Promise does not hang: the host rejects it with `disposed` at the next checkpoint
  h.call(owner, 'slow', []).catch((e) => log('orphan rejected', e.domain, e.code));
  h.destroy(owner);
  try {
    h.call(t, 'complete', ['too late']);
    log('late completion: no error');
  } catch (e) {
    log('late completion error:', e.message);
  }

  try {
    h.call(t, 'slowOf', []);
  } catch (e) {
    log('arity:', e.name, e.message);
  }
  log('sync part done');
  // `sync part done` must come first although the Promises settled during the calls above
  Promise.resolve().then(() => log('done'));
};
