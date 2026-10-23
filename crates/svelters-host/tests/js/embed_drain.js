// FX6b: a hand-written `__svelters_embed` whose calls flush nothing themselves, as the runtime's embed
// since FX6b: what a call changed is applied in a microtask (Svelte's flush), which the host's job
// drain right after the call runs. The drain's errors are the call's.
const h = globalThis.__svelters_host;
const w = h.create('window');
let n = 0;
let recovered = 0;
globalThis.__svelters_recover = () => {
  recovered++;
};
globalThis.__svelters_embed = {
  mount() {},
  props(_id, p) {
    switch (p.mode) {
      case 'set':
        queueMicrotask(() => h.setProp(w, 'title', `n=${++n}`));
        break;
      case 'throw':
        queueMicrotask(() => {
          throw new Error('the flush threw');
        });
        break;
      case 'spin':
        queueMicrotask(() => {
          // queued by the aborted job: runs only after the host recovered, in a later drain
          queueMicrotask(() => h.setProp(w, 'title', `after recovered=${recovered}`));
          for (;;) {}
        });
        break;
      case 'reject':
        Promise.reject(new Error('nobody waits'));
        break;
    }
  },
  info() {},
  unmount() {},
  retarget() {},
  close() {
    return true;
  },
};
export function Panel() {}
