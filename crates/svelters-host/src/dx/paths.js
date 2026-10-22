// svelters:paths (DX1): the app's standard directories and identity, read-only.
const host = globalThis.__svelters_host;
const info = JSON.parse(host.dx('paths', '', ''));

export const paths = Object.freeze({
  config: info.config,
  data: info.data,
  cache: info.cache,
  state: info.state,
  runtime: info.runtime,
});
export const app = Object.freeze({ ...info.app });
