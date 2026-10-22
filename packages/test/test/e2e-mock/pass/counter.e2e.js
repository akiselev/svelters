// Runner self-test on the mock host: seeds, iterations, reset between tests, clock.
import { test, expect } from '../../../index.js';

test.use({ bin: '../../mock-host.mjs' });

test('counts from zero', async ({ app }) => {
  await app.getByRole('button', { name: 'Increment' }).click();
  await expect(app.getByText('Count: 1', { exact: true })).toBeVisible();
});

// The previous test clicked once: the reset between tests gives this one a fresh app.
test('starts fresh after a reset', async ({ app }) => {
  await app.getByRole('button', { name: 'Increment' }).click();
  await expect(app.getByText('Count: 1', { exact: true })).toBeVisible();
});

test('runs once per seed', { seed: 3, iterations: 3 }, async ({ app, seed }) => {
  await expect(app.getByText(`Seed: ${seed}`, { exact: true })).toBeVisible();
  const now = await app.clock.now();
  if (now.seed !== seed) throw new Error(`clock.now seed ${now.seed}, want ${seed}`);
  await app.clock.advance(100);
  if ((await app.clock.now()).ms !== 100) throw new Error('clock.advance did not advance');
});

test.describe('a group', () => {
  test.use({ seed: 9 });
  test('inherits the group seed', async ({ app }) => {
    await expect(app.getByText('Seed: 9', { exact: true })).toBeVisible();
  });
});

test.skip('is skipped', async () => {
  throw new Error('skipped tests do not run');
});
