// The JS layer resolves names to ids once and passes ids afterwards.
const h = globalThis.__svelters_host;
const check = (c, m) => {
  if (!c) throw new Error(m);
};
const kid = h.kindId('thing');
check(typeof kid === 'number' && kid > 0 && h.kindId('thing') === kid, 'kind id is stable');
const t = h.create(kid);
const pid = h.propId(kid, 'spacing');
check(pid === h.propId(kid, 'spacing') && pid > 0, 'prop id is stable');
h.setProp(t, pid, 41);
h.setProp(t, 'spacing', 42);
check(h.getProp(t, pid) === 42, 'ids and names address the same property');
check(h.propType(kid, pid) === 'integer', 'coarse type for attribute coercion');
check(h.propType(kid, h.propId(kid, 'orientation')) === 'enum', 'enum type');
check(h.propType(kid, h.propId(kid, 'loose')) === null, 'no descriptor');
h.setProp(t, h.propId(kid, 'orientation'), 'vertical');
h.listen(t, 'changed', true);
let threw = false;
try {
  h.propId(kid, 'nosuch');
} catch (e) {
  threw = true;
}
check(threw, 'unknown name throws');
const m = h.methodId(kid, 'setBoth');
check(h.call(t, m, [1, 'x']) === true, 'call by id');
h.setProp(t, 'spacing', 42);
console.log('ok');
