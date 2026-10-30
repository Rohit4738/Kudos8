// Controller focus navigation for non-Home pages. OWNER: Session O.
//
// Home has a bespoke two-axis focus model. Settings, the detail page and other
// scrollable pages just need "next control", so this helper walks the visible
// focusable elements inside `.page`, moves DOM focus, scrolls the target into
// view and marks it with `.pad-focus` (programmatic focus does not always match
// `:focus-visible`, and a controller user needs to see where they are).

const FOCUSABLE = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function visible(element: HTMLElement): boolean {
  if (element.offsetParent === null) return false;
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0;
}

function candidates(): HTMLElement[] {
  const root = document.querySelector<HTMLElement>(".page") ?? document.body;
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    visible,
  );
}

function mark(element: HTMLElement): void {
  document
    .querySelectorAll<HTMLElement>(".pad-focus")
    .forEach((node) => node.classList.remove("pad-focus"));
  element.classList.add("pad-focus");
  element.addEventListener(
    "blur",
    () => element.classList.remove("pad-focus"),
    { once: true },
  );
}

/** Moves focus to the next/previous control and keeps it on screen. */
export function focusNext(delta: 1 | -1): void {
  const items = candidates();
  if (items.length === 0) {
    scrollPage(delta);
    return;
  }

  const current = document.activeElement as HTMLElement | null;
  const index = current ? items.indexOf(current) : -1;
  const next =
    index === -1
      ? delta > 0
        ? 0
        : items.length - 1
      : (index + delta + items.length) % items.length;

  const target = items[next];
  target.focus({ preventScroll: true });
  mark(target);
  target.scrollIntoView({ block: "center", behavior: "smooth" });
}

/** Scrolls the page container by roughly one screen when there is nothing to focus. */
export function scrollPage(delta: 1 | -1): void {
  const root = document.querySelector<HTMLElement>(".page");
  if (!root) return;
  root.scrollBy({
    top: delta * root.clientHeight * 0.7,
    behavior: "smooth",
  });
}

/** Activates the focused control, if it is one. */
export function activateFocused(): void {
  const current = document.activeElement as HTMLElement | null;
  if (!current || current === document.body) {
    scrollPage(1);
    return;
  }
  current.click();
}

/** Clears any pad focus marker (called when the mouse takes over). */
export function clearPadFocus(): void {
  document
    .querySelectorAll<HTMLElement>(".pad-focus")
    .forEach((node) => node.classList.remove("pad-focus"));
}
