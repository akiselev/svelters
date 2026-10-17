// Gallery page order and handle layout, shared by check-pages.mjs. The Rust screenshot tests
// (examples/counter/tests/gallery.rs) compute the same thing.
import fs from 'node:fs';
import path from 'node:path';

/** Handles: window #1, outer box #2, sidebar box #3, then one sidebar button per page. */
export const SIDEBAR_FIRST_HANDLE = 4;

/** Page names (file stems) in sidebar order: sorted by file name. */
export function pages(dir) {
  return fs
    .readdirSync(path.join(dir, 'src/pages'))
    .filter((f) => f.endsWith('.svelte'))
    .map((f) => f.slice(0, -'.svelte'.length))
    .sort();
}
