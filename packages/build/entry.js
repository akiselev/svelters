// The bundle entry of an app built by svelteConfig: mounts `src/App.svelte` (the `@app` alias) on the
// host with the native runtime.
import { mount } from '@svelters/native-runtime';
import App from '@app';

mount(App, { props: {} });
