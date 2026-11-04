// Thin layer over the generated bindings: turns tauri-specta result objects into
// thrown errors so TanStack Query can handle them.
import { commands, type CommandError } from './bindings';

export { commands };
export type * from './bindings';

export class BackendError extends Error {
  readonly kind: string;

  constructor(error: CommandError) {
    super(error.message);
    this.name = 'BackendError';
    this.kind = error.kind;
  }
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: CommandError };

export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === 'error') throw new BackendError(r.error);
  return r.data;
}

/** True when running inside the Tauri webview (false in Vitest or a plain browser). */
export const isTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
