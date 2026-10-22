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
  setValue(value: number, opts?: ActOptions): Promise<unknown>;
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
  call<T = any>(method: string, params?: Record<string, unknown>): Promise<T>;
  root(key: string): Locator;
  waitSettled(opts?: { timeout?: number }): Promise<{ frames: number }>;
  snapshot(): Promise<string>;
  health(): Promise<any>;
  interrupt(): Promise<{ interrupted: boolean; running: string | null }>;
  close(): Promise<void>;
}

export function attach(endpoint: string, opts?: { timeout?: number }): Promise<App>;

export function launch(opts: {
  bin: string;
  app?: string;
  args?: string[];
  endpoint?: string;
  seed?: number;
  stepMs?: number;
  headless?: boolean;
  timeout?: number;
  env?: NodeJS.ProcessEnv;
}): Promise<App>;

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
