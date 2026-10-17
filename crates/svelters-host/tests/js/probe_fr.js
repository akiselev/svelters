console.log(typeof FinalizationRegistry, typeof WeakRef, typeof Symbol.dispose, typeof gc);
const reg = new FinalizationRegistry((h) => console.log('finalized', h));
let o = { a: 1 };
reg.register(o, 'held-1', o);
o = null;
let p = { b: 2 };
reg.register(p, 'held-2');
p = null;
