// Callbacks: scopes, release by the native side, owner death, exceptions and re-entrancy.
// The backend invokes the functions synchronously from inside `call`, so all of it runs in an event.
const h = globalThis.__svelters_host;
const t = h.create('thing');
h.listen(t, 'changed', true); // the backend's loop posts one event; the test body runs in it
const log = (...a) => console.log(a.join(' ').trimEnd());
const attempt = (label, f) => {
  try {
    const r = f();
    log(label, 'ok', r === undefined ? '' : JSON.stringify(r));
  } catch (e) {
    log(label + ':', e.name === 'Error' ? e.message : `${e.name} ${e.message}`);
  }
};

globalThis.__svelters_dispatch = () => {
  // call scope: usable during the call, gone afterwards; the result is converted to the return slot.
  log('visit', JSON.stringify(h.call(t, 'visit', [(n) => n * 10])));

  // notified scope: stays registered until the native side releases it.
  let seen = 0;
  h.call(t, 'subscribe', [(n) => { seen += n; }]);
  attempt('fire', () => { h.call(t, 'fire', []); return seen; });
  h.call(t, 'unsubscribe', []);
  try { h.call(t, 'fire', []); log('fire after release: no error'); } catch (e) { log('native-release then fire:', e.message); }

  // the id is generation-tagged: a new registration in the freed slot is a different id
  h.call(t, 'subscribe', [() => {}]);
  try { h.call(t, 'fireOld', []); log('re-register, old id: no error'); } catch (e) { log('re-register, old id:', e.message); }

  // async scope: one invocation, then released
  h.call(t, 'once', [() => {}]);
  attempt('once', () => { h.call(t, 'fireOnce', []); });
  try { h.call(t, 'fireOnce', []); log('once twice: no error'); } catch (e) { log('once twice:', e.message); }

  // forever is refused up front
  try { h.call(t, 'forever', [() => {}]); log('forever: accepted'); } catch (e) { log('forever:', e.name, e.message); }

  // a callback registered by an owner that is destroyed can no longer be called
  const owner = h.create('thing');
  h.call(owner, 'subscribe', [() => {}]);
  h.destroy(owner);
  try { h.call(t, 'fire', []); log('owner gone: no error'); } catch (e) { log('owner gone:', e.message); }

  // an exception in a callback does not unwind into native code
  h.call(t, 'subscribe', [() => { throw new Error('boom from js'); }]);
  try { h.call(t, 'fire', []); log('throwing: no error'); } catch (e) { log('throwing:', e.message); }

  // tree operations are refused while native code waits for the callback
  h.call(t, 'subscribe', [() => { h.create('thing'); }]);
  try { h.call(t, 'fire', []); log('reentrant: no error'); } catch (e) { log('reentrant:', e.message); }

  // only functions are callbacks
  try { h.call(t, 'visit', [5]); } catch (e) { log('bad arg:', e.name, e.message); }
  log('done');
};
