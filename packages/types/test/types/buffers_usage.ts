// E2: read-only typed arrays (`Shared<[T]>` views) and `alloc` type-check as intended.
import { alloc } from '@svelters/types/core';
import type { ReadonlyFloat32Array, ReadonlyBigInt64Array, ReadonlyUint8Array } from '@svelters/types/core';

declare const view: ReadonlyFloat32Array;

// Reads work like on any Float32Array.
const first: number = view[0];
const n: number = view.length;
const sum: number = view.reduce((a, b) => a + b, 0);
for (const x of view) void x;
const part: ReadonlyFloat32Array = view.subarray(1, 3);
// Copies are ordinary, writable arrays.
const copy: Float32Array = view.slice();
copy[0] = 1;
const mapped: Float32Array = view.map((x) => x * 2);

// Writes are type errors (the engine would drop them silently).
// @ts-expect-error: index signature is read-only
view[0] = 1;
// @ts-expect-error: no writing methods
view.fill(0);
// @ts-expect-error: no writing methods
view.set([1, 2]);
// @ts-expect-error: no writing methods
view.sort();

// A read-only view is not a mutable array (a `Buffer<T>` parameter moves a Float32Array).
// @ts-expect-error: missing the writing members
const asMutable: Float32Array = view;

// A mutable array is accepted where a read-only one is expected (a `Shared<[T]>` parameter).
const fromMutable: ReadonlyFloat32Array = new Float32Array(4);

declare const big: ReadonlyBigInt64Array;
const b0: bigint = big[0];
// @ts-expect-error: read-only
big[0] = 1n;

declare const bytes: ReadonlyUint8Array;
const byte: number = bytes[0];

// `alloc` returns the class's instance type.
const f32: Float32Array = alloc(Float32Array, 16);
const i64: BigInt64Array = alloc(BigInt64Array, 2);
// @ts-expect-error: not a typed array class
alloc(Array, 3);

void [first, n, sum, part, copy, mapped, asMutable, fromMutable, b0, byte, f32, i64];
