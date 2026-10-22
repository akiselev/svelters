// RA2: a small consumer of `svelters:app/intent`, type-checked by svelte-check against the dumped
// typings (`.svelters/app/app.d.ts`). Not imported by the app.
import { check, describePick, refused, type Pick, type Refusal, type SlotStatus } from 'svelters:app/intent';

/** Narrows the serde union (typed from its JSON Schema); the switch is exhaustive. */
export function label(s: SlotStatus): string {
  switch (s.kind) {
    case 'empty':
      return 'empty';
    case 'filled':
      return `value ${s.value.toFixed(2)}`;
    case 'refused':
      return `${s.reason}: ${s.message}${s.slot ? ` (${s.slot})` : ''}`;
  }
}

/** Narrows the svelters data enum. */
export function pickIndex(p: Pick): number {
  return p.kind === 'nothing' ? -1 : p.index;
}

export const picks: Pick[] = [{ kind: 'face', index: 1 }, { kind: 'edge', index: 2, bothSides: true }, { kind: 'nothing' }];

export function run(): string[] {
  const r: Refusal = { reason: 'args.missing', message: 'pick a face' };
  return [label(check('face', 1.5)), label(check('edge', null)), label(refused(r)), ...picks.map(describePick)];
}

// @ts-expect-error: not a variant of Pick
export const wrongTag: Pick = { kind: 'vertex', index: 1 };

// @ts-expect-error: `bothSides` is required on an edge
export const missingField: Pick = { kind: 'edge', index: 2 };

// @ts-expect-error: `message` is required on a Refusal
export const missingMessage: Refusal = { reason: 'x' };
