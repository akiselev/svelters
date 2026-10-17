const h = globalThis.__svelters_host;
const t = h.create('thing');
h.listen(t, 'changed', true);
globalThis.__svelters_dispatch = (_h, _signal, args) => {
  const [e, f, r, d, b] = args;
  console.log(JSON.stringify([e, f, r, d.toISOString(), Array.from(b)]));
};
