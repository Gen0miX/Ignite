import '@testing-library/jest-dom/vitest';

// Tests read the French texts: pin the UI language before any module loads.
localStorage.setItem('ignite.language', 'fr');
