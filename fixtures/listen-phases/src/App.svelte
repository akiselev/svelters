<script>
  import { listen } from '@svelters/renderer';
  let log = $state('');
  const add = (s) => (log += s);
</script>

<window title="Phases">
  <box orientation="vertical">
    <label>{log}</label>
    <entry
      onkey-pressed={() => add('b')}
      onkey-pressedcapture={() => add('c')}
      {@attach listen('key-pressed', () => add('a'), { after: true })}
    />
  </box>
</window>
