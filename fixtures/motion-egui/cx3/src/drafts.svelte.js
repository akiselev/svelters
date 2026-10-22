// CX3: a draft bind source in JS (bind-sources.json declares it). cadatomic's would push draft
// events to its command session; this one logs them.
export const drafts = $state({ values: { depth: '5', width: '1' }, log: [] });

/** The marker the template calls in `bind:`; the compiler replaces the call, so it never runs. */
export function slot(path) {
  throw new Error(`slot(${path}) is a bind source marker: bind it, don't call it`);
}

export function draftValue(path) {
  return drafts.values[path] ?? '';
}

export function setDraft(path, value) {
  drafts.log.push(`${path}=${value}`);
  drafts.values[path] = value;
}
