// Runner self-test on the mock host, in TypeScript (Node's type stripping): `mount` and `setProps`.
import { test, expect } from '../../../index.js';

test.use({ bin: '../../mock-host.mjs' });

interface PanelProps {
  title: string;
}

test('mounts a component with props', async ({ mount }) => {
  const props: PanelProps = { title: 'One' };
  const c = await mount('./Panel.svelte', { props });
  await expect(c.getByText('Title: One', { exact: true })).toBeVisible();
  await c.setProps({ title: 'Uno' } satisfies PanelProps);
  await expect(c.getByText('Title: Uno', { exact: true })).toBeVisible();
});
