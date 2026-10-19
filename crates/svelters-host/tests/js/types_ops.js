const h = globalThis.__svelters_host;
const store = h.construct('Gio.ListStore', { 'item-type': 'Gio.File' });
const file = h.callStatic('Gio.File', 'new_for_path', ['/usr/share']);
const none = h.callStatic('Gio.File', 'parse_name', ['x']);
h.call(store, 'append', [file]);
h.setProp(store, 'note', 1);
console.log(JSON.stringify({ store, file, none, chain: h.typeChain(store) }));
try {
  h.construct('Gio.ListStore', 5);
} catch (e) {
  console.log('error: ' + e.message);
}
try {
  h.callStatic('Gio.File', 'parse_name', 'not an array');
} catch (e) {
  console.log('error: ' + e.message);
}
