// Runner self-test on the mock host: failures report their seed and replay command; a test that
// kills or hangs the app fails alone.
import { test, expect } from '../../../index.js';

test.use({ bin: '../../mock-host.mjs' });

test('fails on seed 4', { seed: 3, iterations: 3 }, async ({ app, seed }) => {
  if (seed === 4) await expect(app.getByText('Seed: 40', { exact: true })).toBeVisible();
});

test('crashes the app', async ({ app }) => {
  await app.call('app.crash');
});

test('passes after a crash', async ({ app }) => {
  await app.getByRole('button', { name: 'Increment' }).click();
  await expect(app.getByText('Count: 1', { exact: true })).toBeVisible();
});

test('hangs the app', { timeout: 500 }, async ({ app }) => {
  await app.call('app.hang');
});

test('passes after a hang', async ({ app }) => {
  await expect(app.getByText('Count: 0', { exact: true })).toBeVisible();
});
