const h = globalThis.__svelters_host;
const t = h.create('thing');
const check = (c, m) => {
  if (!c) throw new Error(m);
};
h.setProp(t, 'label', 'plain ascii');
h.setProp(t, 'label', 'latin-1 café');
h.setProp(t, 'label', 'café ✓ 😀');
check(h.getProp(t, 'label') === 'café ✓ 😀', 'round trip');
h.setProp(t, 'label', 'a\u0000b');
check(h.getProp(t, 'label') === 'a\u0000b', 'embedded NUL');
h.setProp(t, 'label', 'café ✓ 😀');
console.log('ok');
