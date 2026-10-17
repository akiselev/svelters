// Keyed diff for the list model: turns "old key order -> new key order" into the smallest useful
// sequence of splices. Applied in order to the old array, the splices produce the new one, so each
// one maps to a single native items-changed / beginInsertRows call.
//
//   removals first (highest range first), then insertions (lowest first).
//
// Keys that are in both lists but out of order are moved: they stay out of the longest increasing
// subsequence, so they are removed and re-inserted with their old id. Keys must be unique.

/** Indices into `seq` of one longest strictly increasing subsequence (patience sorting, O(n log n)). */
export function lis(seq) {
  const tails = []; // tails[k] = index in seq of the smallest tail of an increasing subsequence of length k + 1
  const prev = new Array(seq.length);
  for (let i = 0; i < seq.length; i++) {
    let lo = 0;
    let hi = tails.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (seq[tails[mid]] < seq[i]) lo = mid + 1;
      else hi = mid;
    }
    prev[i] = lo > 0 ? tails[lo - 1] : -1;
    tails[lo] = i;
  }
  const out = new Array(tails.length);
  for (let k = tails.length - 1, i = tails[k]; k >= 0; k--, i = prev[i]) out[k] = i;
  return out;
}

/**
 * @param {readonly unknown[]} oldKeys
 * @param {readonly unknown[]} newKeys
 * @returns {{ pos: number, removed: number, add: unknown[] }[]} splices to apply in order
 */
export function diffKeys(oldKeys, newKeys) {
  const ol = oldKeys.length;
  const nl = newKeys.length;
  let s = 0;
  while (s < ol && s < nl && oldKeys[s] === newKeys[s]) s++;
  let oe = ol;
  let ne = nl;
  while (oe > s && ne > s && oldKeys[oe - 1] === newKeys[ne - 1]) {
    oe--;
    ne--;
  }
  if (s === oe && s === ne) return [];
  if (s === oe) return [{ pos: s, removed: 0, add: newKeys.slice(s, ne) }];
  if (s === ne) return [{ pos: s, removed: oe - s, add: [] }];

  const newPos = new Map();
  for (let j = s; j < ne; j++) newPos.set(newKeys[j], j);
  // Survivors in old order, as their positions in the new order.
  const surv = [];
  const survOld = [];
  for (let i = s; i < oe; i++) {
    const p = newPos.get(oldKeys[i]);
    if (p !== undefined) {
      surv.push(p);
      survOld.push(i);
    }
  }
  const stay = new Set();
  for (const k of lis(surv)) stay.add(oldKeys[survOld[k]]);

  const ops = [];
  // Removals: everything in the middle that is not in the stay set, as maximal ranges, last first.
  let end = -1;
  for (let i = oe - 1; i >= s - 1; i--) {
    const drop = i >= s && !stay.has(oldKeys[i]);
    if (drop) {
      if (end < 0) end = i;
    } else if (end >= 0) {
      ops.push({ pos: i + 1, removed: end - i, add: [] });
      end = -1;
    }
  }
  // Insertions: walk the new middle; stay keys advance the cursor, runs of the rest are inserted.
  let pos = s;
  let run = [];
  for (let j = s; j < ne; j++) {
    const k = newKeys[j];
    if (stay.has(k)) {
      if (run.length) {
        ops.push({ pos, removed: 0, add: run });
        pos += run.length;
        run = [];
      }
      pos++;
    } else {
      run.push(k);
    }
  }
  if (run.length) ops.push({ pos, removed: 0, add: run });
  return ops;
}

/** Applies splices to a copy of `arr` (reference semantics for tests). */
export function applySplices(arr, ops) {
  const out = arr.slice();
  for (const o of ops) out.splice(o.pos, o.removed, ...o.add);
  return out;
}
