// AU2b: the runner on the real host (`svelters-run`, the counter fixture). Run by runner.test.js
// when SVELTERS_RUN points at a built svelters-run, and by linux-gtk CI.
import { test, expect } from '../../index.js';

test.use({ app: '../../../../fixtures/counter-egui', stepMs: 16 });

test('the counter counts', async ({ app }) => {
  const inc = app.getByRole('button', { name: 'Increment' });
  await inc.click();
  await inc.click({ tier: 'semantic' });
  await expect(app.getByText('Count: 2', { exact: true })).toBeVisible();
});

// One app per worker: `app.reset` between tests rebuilds it on a fresh clock.
test('each test starts from a fresh app', async ({ app }) => {
  await expect(app.getByText('Count: 0', { exact: true })).toBeVisible();
  await app.getByRole('button', { name: 'Increment' }).click();
  await expect(app.getByText('Count: 1', { exact: true })).toBeVisible();
});

test.describe('the clock fixture', () => {
  test.use({ app: '../../../../fixtures/counter-egui/src/Clock.svelte' });

  test('a 30 s timer fires after 30 s of virtual time', async ({ app }) => {
    await app.getByRole('button', { name: 'Start' }).click();
    await expect(app.getByText('Fired: no', { exact: true })).toBeVisible();
    await app.clock.advance(30_100);
    await expect(app.getByText('Fired: after 30', { exact: false })).toBeVisible();
  });

  test('the seed replays Math.random', { seed: 5, iterations: 2 }, async ({ app, seed }) => {
    const rolls = await app.getByText('Rolls:').textContent();
    const now = await app.clock.now();
    if (now.seed !== seed) throw new Error(`clock.now seed ${now.seed}, want ${seed}`);
    await app.reset({ seed });
    await expect(app.getByText(rolls, { exact: true })).toBeVisible();
  });
});
