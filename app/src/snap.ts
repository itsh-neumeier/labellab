// Magnetic snapping for the free-layout editor: while moving or resizing a
// box, its edges/center snap to the label start, the tape edges/center and
// the edges/centers of the other boxes, so boxes can be placed flush
// against each other. Pure functions, all values in mm.
import type { Rect } from "./api";

export interface Guides {
  /** Vertical guide lines (x positions). */
  x: number[];
  /** Horizontal guide lines (y positions). */
  y: number[];
}

export interface SnapResult {
  rect: Rect;
  guides: Guides;
}

interface Targets {
  x: number[];
  y: number[];
}

/** Snap lines offered by the label (start, tape edges/center) and the other boxes. */
export function targets(others: Rect[], tapeMm: number): Targets {
  const x = [0];
  const y = [0, tapeMm, tapeMm / 2];
  for (const o of others) {
    x.push(o.x_mm, o.x_mm + o.w_mm, o.x_mm + o.w_mm / 2);
    y.push(o.y_mm, o.y_mm + o.h_mm, o.y_mm + o.h_mm / 2);
  }
  return { x, y };
}

/**
 * Finds the smallest shift that puts one of `points` onto one of `lines`,
 * if within `threshold`. Returns the shift and the line hit.
 */
function nearest(points: number[], lines: number[], threshold: number): { shift: number; line: number } | null {
  let best: { shift: number; line: number } | null = null;
  for (const p of points) {
    for (const l of lines) {
      const shift = l - p;
      if (Math.abs(shift) <= threshold && (!best || Math.abs(shift) < Math.abs(best.shift))) {
        best = { shift, line: l };
      }
    }
  }
  return best;
}

/** Snaps a moved box: its left/right/center to x lines, top/bottom/center to y lines. */
export function snapMove(rect: Rect, t: Targets, threshold: number): SnapResult {
  const out = { ...rect };
  const guides: Guides = { x: [], y: [] };
  const sx = nearest([rect.x_mm, rect.x_mm + rect.w_mm, rect.x_mm + rect.w_mm / 2], t.x, threshold);
  if (sx) {
    out.x_mm += sx.shift;
    guides.x.push(sx.line);
  }
  const sy = nearest([rect.y_mm, rect.y_mm + rect.h_mm, rect.y_mm + rect.h_mm / 2], t.y, threshold);
  if (sy) {
    out.y_mm += sy.shift;
    guides.y.push(sy.line);
  }
  return { rect: out, guides };
}

/** Snaps the right edge (`dx`) and/or bottom edge (`dy`) of a resized box. */
export function snapResize(rect: Rect, t: Targets, threshold: number, dx: boolean, dy: boolean): SnapResult {
  const out = { ...rect };
  const guides: Guides = { x: [], y: [] };
  if (dx) {
    const s = nearest([rect.x_mm + rect.w_mm], t.x, threshold);
    if (s) {
      out.w_mm += s.shift;
      guides.x.push(s.line);
    }
  }
  if (dy) {
    const s = nearest([rect.y_mm + rect.h_mm], t.y, threshold);
    if (s) {
      out.h_mm += s.shift;
      guides.y.push(s.line);
    }
  }
  return { rect: out, guides };
}

/**
 * Rounds every field to 0.1 mm and keeps the box from starting before the
 * label (nothing can be printed there).
 */
export function roundRect(r: Rect): Rect {
  const q = (v: number) => Math.round(v * 10) / 10;
  return { x_mm: Math.max(0, q(r.x_mm)), y_mm: q(r.y_mm), w_mm: q(r.w_mm), h_mm: q(r.h_mm) };
}
