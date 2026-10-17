<script lang="ts">
  interface Props {
    label: string;
    done: boolean;
    ontoggle: () => void;
  }

  let { label, done, ontoggle }: Props = $props();
  const mark = (d: boolean): string => (d ? '✓' : '·');
</script>

<box orientation="horizontal" spacing="4">
  <label>{mark(done)} {label as string}</label>
  <button onclick={ontoggle}>Toggle</button>
</box>
