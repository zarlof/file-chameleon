// Font size for a translated paragraph drawn in the box of the original text.

/** Starting size: about the original text's size (its line height), within readable bounds. */
export function startSize(lineH: number): number {
  return Math.max(10, Math.min(40, Math.round(lineH * 0.8)));
}

/**
 * Shrink `el`'s font until its text fits `maxH` (a little taller than the original box is fine),
 * never below 9 px. Returns the size used.
 */
export function fitText(el: HTMLElement, lineH: number, maxH: number): number {
  let size = startSize(lineH);
  el.style.fontSize = `${size}px`;
  while (size > 9 && el.scrollHeight > maxH) {
    size -= 1;
    el.style.fontSize = `${size}px`;
  }
  return size;
}
