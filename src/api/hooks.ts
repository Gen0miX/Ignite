import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import {
  commands,
  isTauri,
  unwrap,
  type GameUpdate,
  type LibraryQuery,
  type SecretKind,
} from './client';

export const keys = {
  library: (q: LibraryQuery) => ['library', q] as const,
  counts: ['counts'] as const,
  game: (id: string) => ['game', id] as const,
  sources: ['sources'] as const,
  steamSettings: ['steam-settings'] as const,
  secrets: ['secrets'] as const,
};

/** Everything derived from the library must refresh after a change. */
const LIBRARY_KEYS = [['library'], ['counts'], ['game'], ['sources']];

export function useLibraryList(query: LibraryQuery) {
  return useQuery({
    queryKey: keys.library(query),
    queryFn: () => unwrap(commands.libraryList(query)),
    enabled: isTauri(),
    placeholderData: (prev) => prev,
  });
}

export function useLibraryCounts() {
  return useQuery({
    queryKey: keys.counts,
    queryFn: () => unwrap(commands.libraryCounts()),
    enabled: isTauri(),
  });
}

export function useGame(id: string | null) {
  return useQuery({
    queryKey: keys.game(id ?? ''),
    queryFn: () => unwrap(commands.gameGet(id!)),
    enabled: isTauri() && !!id,
  });
}

function useInvalidateLibrary() {
  const client = useQueryClient();
  return () => Promise.all(LIBRARY_KEYS.map((queryKey) => client.invalidateQueries({ queryKey })));
}

export function useUpdateGame(id: string) {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (update: GameUpdate) => unwrap(commands.gameUpdate(id, update)),
    onSuccess: (detail) => {
      client.setQueryData(keys.game(id), detail);
      void invalidate();
    },
  });
}

export function useAddTag(id: string) {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (name: string) => unwrap(commands.gameTagAdd(id, name)),
    onSuccess: () => invalidate(),
  });
}

export function useRemoveTag(id: string) {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (tagId: string) => unwrap(commands.gameTagRemove(id, tagId)),
    onSuccess: () => invalidate(),
  });
}

export function useLaunchGame() {
  return useMutation({
    mutationFn: (gameId: string) => unwrap(commands.launchGame(gameId)),
  });
}

export function useSync() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: () => unwrap(commands.librarySync()),
    onSettled: () => invalidate(),
  });
}

export function useSourcesStatus() {
  return useQuery({
    queryKey: keys.sources,
    queryFn: () => unwrap(commands.sourcesStatus()),
    enabled: isTauri(),
    // Refresh while a background sync is running.
    refetchInterval: (q) => (q.state.data?.some((s) => s.syncing) ? 1500 : false),
  });
}

export function useSteamSettings() {
  return useQuery({
    queryKey: keys.steamSettings,
    queryFn: () => unwrap(commands.steamSettingsGet()),
    enabled: isTauri(),
  });
}

export function useSetSteamId() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (steamId: string | null) => unwrap(commands.steamSettingsSet(steamId)),
    onSuccess: () => client.invalidateQueries({ queryKey: keys.steamSettings }),
  });
}

export function useSetSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ kind, value }: { kind: SecretKind; value: string }) =>
      unwrap(commands.secretsSet(kind, value)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.steamSettings });
      void client.invalidateQueries({ queryKey: keys.secrets });
      void client.invalidateQueries({ queryKey: keys.sources });
    },
  });
}

export function useClearSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (kind: SecretKind) => unwrap(commands.secretsClear(kind)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.steamSettings });
      void client.invalidateQueries({ queryKey: keys.secrets });
      void client.invalidateQueries({ queryKey: keys.sources });
    },
  });
}

/** Refreshes library data whenever the backend reports a change (sync, artwork). */
export function useLibraryEvents() {
  const client = useQueryClient();
  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void listen('library-changed', () => {
      for (const queryKey of LIBRARY_KEYS) void client.invalidateQueries({ queryKey });
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [client]);
}
