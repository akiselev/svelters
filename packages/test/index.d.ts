// @svelters/test (AU2): types for index.js.

import type { ChildProcess } from 'node:child_process';

export class AutomationError extends Error {
  code: string;
  expected?: string;
  actual?: string;
}

export function parseEndpoint(endpoint: string): { path: string } | { host: string; port: number };

export class Client {
  static connect(endpoint: string, opts?: { timeout?: number }): Promise<Client>;
  call<T = any>(method: string, params?: Record<string, unknown>): Promise<T>;
  close(): void;
}

export interface ActOptions {
  tier?: 'synthetic' | 'semantic';
  force?: boolean;
  /** Virtual ms on the app's clock. */
  timeout?: number;
}

export interface NodeInfo {
  ref: { id: number; epoch: number };
  tag: string;
  role: string;
  name: string | null;
  text: string | null;
  value: string | number | null;
  anchor: string | null;
  root: string | null;
  rect: [number, number, number, number] | null;
  states: string[];
}

export class Locator {
  readonly app: App;
  readonly selector: string;
  locator(selector: string): Locator;
  getByRole(role: string, opts?: { name?: string; exact?: boolean }): Locator;
  getByText(text: string, opts?: { exact?: boolean }): Locator;
  getByLabel(label: string): Locator;
  nth(i: number): Locator;
  first(): Locator;
  click(opts?: ActOptions): Promise<unknown>;
  hover(opts?: ActOptions): Promise<unknown>;
  focus(opts?: ActOptions): Promise<unknown>;
  fill(text: string, opts?: ActOptions): Promise<unknown>;
  setValue(value: number, opts?: ActOptions & { handle?: string | number }): Promise<unknown>;
  all(): Promise<NodeInfo[]>;
  count(): Promise<number>;
  info(): Promise<NodeInfo>;
  textContent(): Promise<string | null>;
  snapshot(): Promise<string>;
}

export class App extends Locator {
  readonly client: Client;
  readonly child: ChildProcess | null;
  readonly endpoint: string;
  readonly hello: any;
  readonly keyboard: { press(key: string): Promise<unknown>; type(text: string): Promise<unknown> };
  /** AU1b's clock: `advance`/`runFor` need the `clock` grant and a virtual clock. */
  readonly clock: {
    now(): Promise<ClockNow>;
    advance(ms: number): Promise<{ frames: number; ms: number }>;
    runFor(ms: number): Promise<{ frames: number; ms: number }>;
  };
  readonly grants: { read?: boolean; act?: boolean; tooling?: boolean; clock?: boolean };
  /** `app.reset`: the start app in a fresh host on a fresh virtual clock. */
  reset(opts?: { seed?: number; timeout?: number }): Promise<{ seed: number | null; frames: number }>;
  /** `app.mount`: one component alone in a fresh host (compiled live). */
  mount(component: string | URL, opts?: MountOptions): Promise<Mounted>;
  call<T = any>(method: string, params?: Record<string, unknown>): Promise<T>;
  root(key: string): Locator;
  waitSettled(opts?: { timeout?: number }): Promise<{ frames: number }>;
  snapshot(): Promise<string>;
  health(): Promise<any>;
  interrupt(): Promise<{ interrupted: boolean; running: string | null }>;
  close(): Promise<void>;
}

export interface ClockNow {
  mode: 'virtual' | 'real';
  ms: number | null;
  seed: number | null;
  session_ms: number;
}

export interface MountOptions {
  /** The component's export name (default: the file stem). */
  export?: string;
  props?: Record<string, unknown>;
  size?: [number, number];
  seed?: number;
  timeout?: number;
}

export class Mounted extends Locator {
  readonly module: string;
  setProps(props: Record<string, unknown>, opts?: { timeout?: number }): Promise<{ frames: number }>;
}

export function attach(endpoint: string, opts?: { timeout?: number }): Promise<App>;

export function launch(opts: {
  bin: string;
  app?: string;
  args?: string[];
  clock?: 'virtual' | 'real';
  endpoint?: string;
  seed?: number;
  stepMs?: number;
  headless?: boolean;
  timeout?: number;
  env?: NodeJS.ProcessEnv;
  stderr?: 'inherit' | 'pipe' | 'ignore';
}): Promise<App>;

/** Options of `test.use`, `test.describe` scopes and a test's own second argument. */
export interface TestOptions {
  /** An app dir, `App.svelte` or `dist/app.js`; relative to the test file. None: an empty host for `mount`. */
  app?: string;
  bin?: string;
  clock?: 'virtual' | 'real';
  stepMs?: number;
  args?: string[];
  seed?: number;
  iterations?: number;
  /** Wall ms per iteration (default 30 000). */
  timeout?: number;
  retries?: number;
}

export interface Fixtures {
  /** The worker's app, reset to `seed` before the test. */
  app: App;
  /** `app.mount` with the test's seed; relative paths are the test file's. */
  mount(component: string | URL, opts?: MountOptions): Promise<Mounted>;
  seed: number;
}

type TestFn = (fixtures: Fixtures) => Promise<void> | void;
export interface TestRegister {
  (title: string, fn: TestFn): void;
  (title: string, opts: TestOptions, fn: TestFn): void;
}
export interface TestApi extends TestRegister {
  skip: TestRegister;
  only: TestRegister;
  use(opts: TestOptions): void;
  describe(title: string, fn: () => void): void;
}
/** svelters' own runner (`svelters-e2e`, AU2b). */
export const test: TestApi;

export function dedent(text: string): string;

type WaitOpts = { timeout?: number };
export function expect(locator: Locator): {
  toBeVisible(o?: WaitOpts): Promise<unknown>;
  toBeHidden(o?: WaitOpts): Promise<unknown>;
  toBeAttached(o?: WaitOpts): Promise<unknown>;
  toBeDetached(o?: WaitOpts): Promise<unknown>;
  toBeEnabled(o?: WaitOpts): Promise<unknown>;
  toHaveText(text: string, o?: WaitOpts & { exact?: boolean }): Promise<unknown>;
  toContainText(text: string, o?: WaitOpts): Promise<unknown>;
  toHaveValue(value: string | number, o?: WaitOpts & { exact?: boolean }): Promise<unknown>;
  toHaveCount(count: number, o?: WaitOpts): Promise<unknown>;
  toMatchSceneSnapshot(expected: string, o?: WaitOpts): Promise<void>;
};
