import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator';
import { BASE, SITE, REPO, REPO_BRANCH } from './site.config.mjs';

const guide = (slug, label) => ({ slug, label });

export default defineConfig({
  site: SITE,
  base: BASE,
  trailingSlash: 'always',
  build: { concurrency: 4 },
  vite: { build: { sourcemap: false } },
  integrations: [
    starlight({
      title: 'Svelters',
      description: 'Stock Svelte 5 rendered by native toolkits: GTK 4, Qt 6 and more.',
      social: [{ icon: 'github', label: 'GitHub', href: REPO }],
      editLink: { baseUrl: `${REPO}/edit/${REPO_BRANCH}/docs/site/` },
      customCss: ['./src/styles/custom.css'],
      components: { ThemeSelect: './src/components/ThemeSelect.astro' },
      tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
      plugins: [starlightLinksValidator({ errorOnRelativeLinks: false, errorOnInvalidHashes: true, errorOnLocalLinks: true })],
      sidebar: [
        {
          label: 'Guides',
          items: [
            guide('guides/getting-started', 'Getting started'),
            guide('guides/architecture', 'Architecture'),
            guide('guides/svelte-support', 'Svelte feature support'),
            guide('guides/lists-and-snippets', 'Snippets, lists, virtual lists'),
            guide('guides/native-models', 'Native models in lists'),
            guide('guides/events', 'Events and modifiers'),
            guide('guides/bindings', 'Bindings'),
            guide('guides/rust-exports', 'Rust exports'),
            guide('guides/styling', 'Styling'),
            guide('guides/annotation-workflow', 'Metadata and annotations'),
            guide('guides/platforms', 'Platform status'),
          ],
        },
        {
          label: 'Concepts',
          items: [guide('concepts/ownership', 'Ownership and lifetimes'), guide('concepts/reactivity', 'Reactivity of native properties'), guide('concepts/abi', 'Backend ABI reference'), guide('concepts/docs-generator', 'How these docs are made')],
        },
        {
          label: 'API reference',
          items: [
            guide('api', 'Overview'),
            guide('api/matrix', 'Support matrix'),
            { label: 'GTK 4 elements', slug: 'api/gtk' },
            { label: 'GTK 4 types', slug: 'api/gtk/types' },
            { label: 'Qt 6 elements', slug: 'api/qt' },
            { label: 'Qt 6 types', slug: 'api/qt/types' },
            { label: 'GTK 4 styling properties', slug: 'api/styling/gtk' },
            { label: 'Qt 6 styling properties', slug: 'api/styling/qt' },
            { label: 'AppKit styling properties', slug: 'api/styling/appkit' },
            { label: 'WinUI 3 styling properties', slug: 'api/styling/winui' },
            guide('coverage', 'Coverage dashboard'),
          ],
        },
        { label: 'Non-UI types (GTK)', items: [{ autogenerate: { directory: 'types/gtk' } }] },
        { label: 'Non-UI types (Qt)', items: [{ autogenerate: { directory: 'types/qt' } }] },
        { label: 'Non-UI types (AppKit)', items: [{ autogenerate: { directory: 'types/appkit' } }] },
        { label: 'Non-UI types (WinUI)', items: [{ autogenerate: { directory: 'types/winui' } }] },
        { label: 'Project', items: [guide('about/licensing', 'Licensing of documentation text')] },
      ],
    }),
  ],
});
