const h = globalThis.__svelters_host;
const t = h.create('thing');
h.setProp(t, 'meta', variant('a{sv}', { n: variant('(is)', [1, 'x']), list: [1, 2] }));
let msg = '';
try {
  variant('', 1);
} catch (e) {
  msg = e.name;
}
console.log('empty signature:', msg);
console.log('class:', new NativeVariant('i', 3).signature, variant('i', 3) instanceof NativeVariant);
