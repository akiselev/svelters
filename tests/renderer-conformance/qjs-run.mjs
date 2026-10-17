// usage: qjs --std qjs-run.mjs <absolute path of bundle>
import './qjs-prelude.mjs';
await import(scriptArgs[1]);
