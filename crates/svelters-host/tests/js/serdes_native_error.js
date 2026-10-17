// A GError-style failure arrives as a NativeError with the domain, an enum-name code and the message.
const h = globalThis.__svelters_host;
const t = h.create('thing');
try {
  h.call(t, 'gerror', []);
  console.log('no error');
} catch (e) {
  console.log([e.name, e.domain, e.code, e.message, e instanceof Error, e instanceof NativeError].join('|'));
}
try {
  h.call(t, 'fail', []);
} catch (e) {
  console.log([e.name, e.message, e instanceof NativeError].join('|'));
}
