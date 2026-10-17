import { KIND } from '../kinds.js';

export function make(h) {
  const w = h.create(KIND);
  h.setProp(w, 'title', 'imported');
}
