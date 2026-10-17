// Shared constants for astro.config.mjs and the page generator (scripts/).
export const SITE = process.env.DOCS_SITE ?? 'https://akiselev.github.io';
// Base path with a leading and trailing slash ("/" or "/svelters/").
export const BASE = (() => {
  let b = process.env.DOCS_BASE ?? '/';
  if (!b.startsWith('/')) b = '/' + b;
  if (!b.endsWith('/')) b += '/';
  return b;
})();
export const REPO = 'https://github.com/akiselev/svelters';
export const REPO_BRANCH = 'master';

// Toolkits whose frontends do not exist yet. They get a "planned" column in the support matrix.
export const PLANNED_TOOLKITS = [
  { id: 'winui', name: 'WinUI 3', status: 'research' },
  { id: 'appkit', name: 'AppKit / UIKit', status: 'research' },
  { id: 'egui', name: 'egui', status: 'planned' },
];

export const TOOLKIT_ORDER = ['gtk', 'qt'];
