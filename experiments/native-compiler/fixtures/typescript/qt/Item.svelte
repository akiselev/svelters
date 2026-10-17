<script lang="ts">
  interface Props {
    label: string;
    done: boolean;
    ontoggle: () => void;
  }

  let { label, done, ontoggle }: Props = $props();
  const mark = (d: boolean): string => (d ? '✓' : '·');
</script>

<box-layout direction="left-to-right" spacing="4">
  <label>{mark(done)} {label as string}</label>
  <push-button onclicked={ontoggle}>Toggle</push-button>
</box-layout>
