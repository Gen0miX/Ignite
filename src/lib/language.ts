// Applying the UI language: `<html lang>`, the backend copy (notification-area
// menu, language asked to Steam) and switching with a reload.

import { commands, isTauri, unwrap } from '../api/client';
import { language, storeLanguage, type Language } from '../i18n';

/** At startup: tags the document and tells the backend the active language. */
export function applyLanguage() {
  document.documentElement.lang = language;
  if (isTauri()) {
    // Best effort: a failure only leaves the tray menu in the previous language.
    void commands.languageSet(language).catch(() => undefined);
  }
}

/** Saves the choice and reloads the window so every text follows. */
export async function changeLanguage(next: Language) {
  if (next === language) return;
  if (isTauri()) await unwrap(commands.languageSet(next));
  storeLanguage(next);
  window.location.reload();
}
