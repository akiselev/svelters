// Svelte passes static attributes as strings; the descriptors decide what they mean.
const h = globalThis.__svelters_host;
const t = h.create('thing');
h.setProp(t, 'spacing', '8');
h.setProp(t, 'ratio', '0.5');
h.setProp(t, 'enabled', '');
h.setProp(t, 'orientation', 'vertical');
h.setProp(t, 'align', 'left vcenter');
h.setProp(t, 'label', '8');
console.log('done');
