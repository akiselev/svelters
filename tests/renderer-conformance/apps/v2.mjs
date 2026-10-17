import * as R from '#renderer';
import { report } from '../harness.mjs';
import V from './Values2.svelte';
await report(R, 'Values2 (no spread)', V, async (c) => c.step('inc', () => c.app.inc()));
