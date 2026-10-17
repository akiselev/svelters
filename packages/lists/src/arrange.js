// Sort, filter and group for GtkListBox / GtkFlowBox (and any other container that shows its children in
// document order). The native sort, filter and header funcs are not exposed: they would call JS once per
// pair of rows while GTK lays out. Following the model-view decision (docs/notes/m5-lists.md), JS arranges the
// data and the app renders the result keyed, so GTK only sees inserts, removes and moves.

/**
 * @template T
 * @param {readonly T[]} items
 * @param {{ filter?: (item: T) => boolean, compare?: (a: T, b: T) => number, group?: (item: T) => string | null | undefined }} [opts]
 * @returns {{ item: T, header: string }[]} the rows to render, in order. `header` is the section header of the
 *   first row of each group (`parent:header={row.header}`) and "" for the others, so it can be passed straight
 *   to the attribute (an empty header means none).
 */
export function arrange(items, { filter, compare, group } = {}) {
  let rows = filter ? items.filter(filter) : items.slice();
  if (compare) rows = rows.sort(compare);
  let last;
  return rows.map((item) => {
    if (!group) return { item, header: '' };
    const g = group(item) ?? '';
    const header = g !== last ? g : '';
    last = g;
    return { item, header };
  });
}
