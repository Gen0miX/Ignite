import type { ComponentType } from 'react';
import { AppWindow, Gamepad2, Joystick, type LucideProps } from 'lucide-react';
import {
  siBattledotnet,
  siEa,
  siEpicgames,
  siGogdotcom,
  siRiotgames,
  siSteam,
  siUbisoft,
} from 'simple-icons';
import type { SourceKind } from '../../api/client';

/**
 * Brand marks from Simple Icons (CC0). Simple Icons has no Xbox mark
 * (Microsoft asked for its removal), and local executables and emulators are
 * not brands: those use a Lucide icon.
 */
const BRANDS: Partial<Record<SourceKind, string>> = {
  steam: siSteam.path,
  epic: siEpicgames.path,
  gog: siGogdotcom.path,
  battlenet: siBattledotnet.path,
  ea: siEa.path,
  ubisoft: siUbisoft.path,
  riot: siRiotgames.path,
};

const FALLBACK: Record<string, ComponentType<LucideProps>> = {
  xbox: Gamepad2,
  local: AppWindow,
  emulator: Joystick,
};

/** Platform logo in `currentColor`. Decorative: the name is always given next to it. */
export function SourceIcon({ kind, size = 14 }: { kind: SourceKind; size?: number }) {
  const path = BRANDS[kind];
  if (path) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        aria-hidden="true"
        focusable="false"
      >
        <path d={path} />
      </svg>
    );
  }
  const Icon = FALLBACK[kind] ?? AppWindow;
  return <Icon size={size} strokeWidth={2.25} aria-hidden="true" />;
}
