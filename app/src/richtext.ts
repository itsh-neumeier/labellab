// Inline text styles: `**bold**` and `__italic__` inside a text element
// (rendered by `ll_render::richtext`; unpaired markers stay literal there).

export const BOLD_MARK = "**";
export const ITALIC_MARK = "__";

/** The text without style markers (for captions and names). */
export function stripMarkup(text: string): string {
  return text.replace(/\*\*([\s\S]*?)\*\*/g, "$1").replace(/__([\s\S]*?)__/g, "$1");
}

/**
 * Wraps the selection of `area` in `mark`, or removes the marks if the
 * selection is already wrapped. Returns false (and changes nothing) when
 * nothing is selected.
 */
export function toggleMark(area: HTMLTextAreaElement, mark: string): boolean {
  const { selectionStart: start, selectionEnd: end, value } = area;
  if (start === end) return false;
  const selected = value.slice(start, end);
  const n = mark.length;
  let next: string;
  let from: number;
  let to: number;
  if (selected.length >= 2 * n && selected.startsWith(mark) && selected.endsWith(mark)) {
    next = value.slice(0, start) + selected.slice(n, -n) + value.slice(end);
    [from, to] = [start, end - 2 * n];
  } else if (value.slice(start - n, start) === mark && value.slice(end, end + n) === mark) {
    next = value.slice(0, start - n) + selected + value.slice(end + n);
    [from, to] = [start - n, end - n];
  } else {
    next = value.slice(0, start) + mark + selected + mark + value.slice(end);
    [from, to] = [start + n, end + n];
  }
  area.value = next;
  area.focus();
  area.setSelectionRange(from, to);
  area.dispatchEvent(new Event("input"));
  return true;
}
