import renderer, { listen } from '@svelters/renderer';
import { Canvas2D } from './recorder.js';

// The size arrives as `resize(width, height)` on GTK and `resize(Size, oldSize)` on Qt.
function sizeOf(a, b) {
  return typeof a === 'number' ? { width: a, height: b } : { width: a?.width ?? 0, height: a?.height ?? 0 };
}

/**
 * An attachment for `<canvas>` (GTK: the drawing area; Qt: the canvas element) that draws with a
 * Canvas2D-like context: `<canvas content-width={200} content-height={100} {@attach draw((ctx, size) => { ... })} />`.
 *
 * `fn(ctx, {width, height})` records into `ctx`; the list is sent as one value and replayed natively. It
 * runs in an effect: reactive state it reads invalidates the drawing and redraws it, and so does a resize
 * (the size is `{width: 0, height: 0}` until the toolkit reports it). Nothing runs while the toolkit paints.
 */
export function draw(fn) {
  return (node) => {
    let size = $state.raw({ width: 0, height: 0 });
    const attach = listen('resize', (...a) => {
      // Qt posts (size, oldSize); GTK posts (width, height). A listener never gets an event object here.
      size = sizeOf(...a);
    });
    const stop = attach(node);
    $effect(() => {
      const ctx = new Canvas2D(size.width, size.height);
      fn(ctx, size);
      renderer.setAttribute(node, 'ops', ctx.toJSON());
    });
    return stop;
  };
}
