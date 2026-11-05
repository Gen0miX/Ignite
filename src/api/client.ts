// Thin layer over the generated bindings: turns tauri-specta result objects into
// thrown errors so TanStack Query can handle them, and exposes friendly type names.
import { convertFileSrc } from '@tauri-apps/api/core';
import * as b from './bindings';

export const commands = b.commands;

export type CommandError = b.CommandError;
export type GameSummary = b.GameSummary_Serialize;
export type GameDetail = b.GameDetail_Serialize;
export type GameStatus = b.GameStatus;
export type SourceKind = b.SourceKind;
export type LibraryView = b.LibraryView;
export type SortKey = b.SortKey;
export type LibraryQuery = b.LibraryQuery_Deserialize;
export type LibraryCounts = b.LibraryCounts;
export type GameUpdate = b.GameUpdate_Deserialize;
export type SourceCard = b.SourceCard;
export type SourceStatus = b.SourceStatus;
export type SyncReport = b.SyncReport;
export type SteamSettings = b.SteamSettings;
export type SecretKind = b.SecretKind;
export type SecretStatus = b.SecretStatus;
export type Tag = b.Tag;
export type PlaySession = b.PlaySession_Serialize;
export type SessionInput = b.SessionInput_Deserialize;
export type SessionOrigin = b.SessionOrigin;
export type Goal = b.Goal_Serialize;
export type AchievementsView = b.AchievementsView_Serialize;
export type AchievementItem = b.AchievementItem_Serialize;
export type ActiveGame = b.ActiveGame;
export type Progress = b.Progress;
export type ImportScan = b.ImportScan_Serialize;
export type ImportCandidate = b.ImportCandidate_Serialize;
export type ImportDecision = b.ImportDecision;
export type ImportReport = b.ImportReport;
export type ExeInfo = b.ExeInfo_Serialize;
export type ArtworkSlot = b.ArtworkSlot;

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

/** URL for an image of the artwork cache (served by Tauri's `asset:` protocol). */
export function artworkSrc(path: string | null | undefined): string | undefined {
  if (!path || !isTauri()) return undefined;
  return convertFileSrc(path);
}
