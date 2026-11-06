/** Nearest ancestor that scrolls vertically (the page area of the shell). */
function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const { overflowY } = getComputedStyle(p);
    if ((overflowY === 'auto' || overflowY === 'scroll') && p.scrollHeight > p.clientHeight) {
      return p;
    }
  }
  return null;
}

/**
 * Scrolls only the page's own scroll area to the section. A plain `#anchor`
 * jump (or `scrollIntoView`) also scrolls the `overflow: hidden` ancestors of
 * the shell, which leaves the window shifted and cut off.
 */
export function scrollToSection(id: string): void {
  const target = document.getElementById(id);
  if (!target) return;
  const container = scrollParent(target);
  if (container) {
    const top =
      target.getBoundingClientRect().top -
      container.getBoundingClientRect().top +
      container.scrollTop -
      12;
    container.scrollTo({ top, behavior: 'smooth' });
  }
  const focusable = target.closest('section') ?? target;
  if (!focusable.hasAttribute('tabindex')) focusable.setAttribute('tabindex', '-1');
  focusable.focus({ preventScroll: true });
}
