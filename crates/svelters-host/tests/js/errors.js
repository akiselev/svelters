const h = globalThis.__svelters_host;
const w = h.create('window');
h.listen(w, 'closed', true);

console.log('hello', { a: 1 }, [1, 2], null, undefined);
console.error('bad', 42);

setTimeout(() => {
  throw new Error('boom in timer');
}, 1);
Promise.reject(new Error('unhandled one'));

globalThis.__svelters_dispatch = () => {
  throw new TypeError('boom in dispatch');
};
