const h = globalThis.__svelters_host;
const t = h.create('thing');
const show = (name, v) => console.log(name, JSON.stringify(v));
// OUT parameters are not passed; the result is {ret, ...outs}.
show('measure', h.call(t, 'measure', [2.5]));
// INOUT parameters are passed and come back updated; a void method has no `ret`.
show('bump', h.call(t, 'bump', [7, 'hi']));
show('origin', h.call(t, 'origin', []));
try {
  h.call(t, 'measure', [1, 2, 3]);
} catch (e) {
  console.log('arity', e.name, e.message);
}
try {
  h.call(t, 'badOuts', []);
} catch (e) {
  console.log('bad answer', e.name, e.message);
}
