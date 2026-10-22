// AU2b: `mount` on the real host (E2E-INSPECTOR §2.4): one component alone in a fresh host,
// compiled live, then new props with its state kept.
import { test, expect } from '../../index.js';

test('Panel counts and takes new props', async ({ mount }) => {
  const c = await mount('../../../../fixtures/counter-egui/src/Panel.svelte', { props: { title: 'One' }, size: [320, 200] });
  await c.getByRole('button', { name: 'Inc One' }).click();
  await expect(c.getByText('One clicks: 1', { exact: true })).toBeVisible();
  await c.setProps({ title: 'Uno' });
  await expect(c.getByRole('button', { name: 'Inc Uno' })).toBeVisible();
  await expect(c.getByText('Uno clicks: 1', { exact: true })).toBeVisible();
});

test('a mounted component runs on the virtual clock', async ({ mount, app }) => {
  await mount('../../../../fixtures/counter-egui/src/Panel.svelte');
  const before = (await app.clock.now()).ms;
  await app.clock.advance(1_000);
  const after = (await app.clock.now()).ms;
  if (!(after - before >= 1_000)) throw new Error(`clock moved ${after - before} ms`);
  await expect(app.getByText('Panel clicks: 0', { exact: true })).toBeVisible();
});
