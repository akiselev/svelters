#!/usr/bin/env node
// svelters' own e2e runner (AU2b): see ../runner/cli.js.
import { main } from '../runner/cli.js';

process.exitCode = await main();
