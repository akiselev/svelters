<!-- kinds: Box Label Button Entry -->
<script>
  // The themed variant of the styling page: the dark colour scheme and a token override, set while
  // the page is shown and undone when it goes away.
  import { setColorScheme, setTokens } from '@svelters/native-runtime/style';
  import StyledDemo from '../StyledDemo.svelte';

  $effect(() => {
    setColorScheme('dark');
    setTokens({ '--sv-accent': '#e5a50a', '--sv-accent-fg': '#1e1e1e', '--sv-accent-hover': '#f5c211' });
    return () => {
      setColorScheme('light');
      setTokens({ '--sv-accent': null, '--sv-accent-fg': null, '--sv-accent-hover': null });
    };
  });
</script>

<StyledDemo />
