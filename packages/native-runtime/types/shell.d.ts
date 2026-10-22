// `svelters:shell` and `@svelters/native-runtime/shell` (DX2): hand URLs and files to the system.

/** Opens `url` with its handler (the browser, the mail client). */
export function openUrl(url: string): Promise<void>;
/** Opens the file or directory at the absolute `path` with the system's default application. */
export function openPath(path: string): Promise<void>;
/** Shows the absolute `path` selected in the file manager. */
export function revealInFolder(path: string): Promise<void>;

export interface Shell {
  openUrl: typeof openUrl;
  openPath: typeof openPath;
  revealInFolder: typeof revealInFolder;
}
export const shell: Shell;
export default shell;
