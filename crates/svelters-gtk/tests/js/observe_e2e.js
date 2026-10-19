// ABI v2.6 on GTK with the real host: a native edit of an observed TextBuffer reaches
// __svelters_prop_changed(handle, prop); the handler reads the value live and stops observing.
// Handles in creation order: buffer 1, window 2, label 3.
const h = globalThis.__svelters_host;
const buf = h.construct('Gtk.TextBuffer', { text: 'a' });
const win = h.create('window');
const label = h.create('label');
h.insert(win, label, null);
h.call(win, 'present', []);

const log = [];
globalThis.__svelters_dispatch = () => {};
globalThis.__svelters_prop_changed = (handle, prop) => {
  log.push(`#${handle} ${prop}=${h.getProp(handle, prop)}`);
  h.setProp(label, 'label', log.join('|'));
  h.observe(handle, prop, false);
};
h.setProp(label, 'label', `observed=${h.observe(buf, 'text', true)}`);
