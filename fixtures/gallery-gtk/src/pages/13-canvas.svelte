<!-- kinds: DrawingArea -->
<script>
  // Recording canvas (X8): the attachment records a Canvas2D-like display list in an effect and sends it as one
  // value; the drawing area replays it with cairo. Reading `step` makes the button redraw it. The size comes
  // from the drawing area's `resize` signal (the list is drawn once at 0x0, then again at the real size).
  import { draw } from '@svelters/canvas';
  let step = $state(0);
</script>

<box orientation="vertical" spacing={6}>
  <label>canvas: redraws {step}</label>
  <button label="redraw" onclick={() => step++} />
  <canvas
    contentWidth={240}
    contentHeight={120}
    {@attach draw((ctx, { width, height }) => {
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
      ctx.font = 'bold 14px sans-serif';
      ctx.fillText(`${width}x${height} draw ${step}`, 10, 90);
    })}
  />
</box>
