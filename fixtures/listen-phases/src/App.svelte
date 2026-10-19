<script>
  import { listen } from '@svelters/native-runtime';
  let log = $state('');
  const add = (s) => (log += s);
</script>

<window title="Phases">
  <box orientation="vertical">
    <label>{log}</label>
    <entry>
      <event-controller-key
        onkey-pressed={() => add('b')}
        onkey-pressedcapture={() => add('c')}
        {@attach listen('key-pressed', () => add('a'), { after: true })}
      />
    </entry>
  </box>
</window>
