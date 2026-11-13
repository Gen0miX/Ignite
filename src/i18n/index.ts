// Active UI language. The choice lives in localStorage so it is known before
// the first render (and in the popup window, same origin); the backend keeps a
// copy for the texts it shows itself (notification-area menu). Changing it
// reloads the window: every module then reads the new dictionary.

import { en } from './en';
import { fr, type Strings } from './fr';

export type Language = 'fr' | 'en';

export const LANGUAGES: { id: Language; label: string }[] = [
  { id: 'fr', label: 'Français' },
  { id: 'en', label: 'English' },
];

const STORAGE_KEY = 'ignite.language';

const isLanguage = (v: unknown): v is Language => v === 'fr' || v === 'en';

/** Stored choice, else the system language (French or English). */
export function detectLanguage(): Language {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isLanguage(stored)) return stored;
  } catch {
    // Storage unavailable: fall back to the system language.
  }
  const system = typeof navigator === 'undefined' ? 'fr' : navigator.language;
  return system.toLowerCase().startsWith('fr') ? 'fr' : 'en';
}

export function storeLanguage(lang: Language) {
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // Not persisted: the next start detects the language again.
  }
}

export const language: Language = detectLanguage();

/** UI texts in the active language. */
export const strings: Strings = language === 'en' ? en : fr;

/** BCP 47 locale for `Intl` (dates, numbers, sorting). */
export const locale = language === 'en' ? 'en-GB' : 'fr-FR';

/** Genres and player modes are stored with French names: translate for display. */
export const genreLabel = (name: string): string =>
  (strings.genreNames as Record<string, string>)[name] ?? name;

export type { Strings };
