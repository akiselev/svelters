<!-- kinds: Image ProgressBar Spinner Canvas -->
<script>
  // WG2b display kinds. The image is `bytes://gallery/dot.png`, which tests/gallery.rs registers
  // (an app registers its own with `Context::include_bytes`, or uses a file path). The canvas draws
  // with `@svelters/canvas`, the same code as the GTK and Qt galleries; its size comes from `resize`.
  import { draw } from '@svelters/canvas';
  let fraction = $state(0.25);
  let busy = $state(true);
  let size = $state('0x0');
</script>

<box gap={6}>
  <image source="bytes://gallery/dot.png" accessible-name="Dot" align-self="start" />
  <progress-bar {fraction} show-percentage accessible-name="Upload" />
  <button onclicked={() => (fraction = Math.min(1, fraction + 0.25))}>More</button>
  <box flex-direction="row" gap={6} align-items="center">
    <spinner spinning={busy} accessible-name="Loading" />
    <button onclicked={() => (busy = !busy)}>Toggle spinner</button>
  </box>
  <label>canvas: {size}</label>
  <canvas
    content-width={240}
    content-height={120}
    accessible-name="Drawing"
    align-self="start"
    {@attach draw((ctx, { width, height }) => {
      size = `${width}x${height}`;
      ctx.fillStyle = '#e01b24';
      ctx.fillRect(10, 10, 80, 60);
      ctx.fillStyle = '#3584e4';
      ctx.beginPath();
      ctx.arc(160, 50, 30, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = '#33d17a';
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.moveTo(10, 100);
      ctx.lineTo(230, 100);
      ctx.stroke();
      ctx.fillStyle = '#000000';
      ctx.font = '14px sans-serif';
      ctx.fillText(`${width}x${height}`, 10, 90);
    })}
  />
</box>
