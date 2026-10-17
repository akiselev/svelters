const h = globalThis.__svelters_host;
const lbl = h.create('label');
const log = [];
let first;
requestAnimationFrame(() => log.push('a'));
requestAnimationFrame((t) => {
  first = t;
  log.push('b');
  // Requested during a frame: runs in the next frame, not this one.
  requestAnimationFrame((t2) => {
    log.push('c');
    h.setProp(lbl, 'label', `${log.join(',')} ${t2 >= first} ${typeof t2}`);
  });
});
const cancelled = requestAnimationFrame(() => log.push('cancelled'));
cancelAnimationFrame(cancelled);
requestAnimationFrame(() => {
  throw new Error('frame boom');
});
requestAnimationFrame(() => log.push('d'));
