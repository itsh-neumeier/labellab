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
/** Which edges of a box a resize handle moves. */
export interface Edges {
  left: boolean;
  right: boolean;
  top: boolean;
  bottom: boolean;
}

/** Edges moved by a handle named by compass letters ("n", "se", "w", …). */
export function handleEdges(handle: string): Edges {
  return {
    left: handle.includes("w"),
    right: handle.includes("e"),
    top: handle.includes("n"),
    bottom: handle.includes("s"),
  };
}

/** Snaps the moving edges of a resized box; the opposite edges stay put. */
export function snapResize(rect: Rect, t: Targets, threshold: number, edges: Edges): SnapResult {
  const out = { ...rect };
  const guides: Guides = { x: [], y: [] };
  if (edges.right) {
    const s = nearest([rect.x_mm + rect.w_mm], t.x, threshold);
    if (s) {
      out.w_mm += s.shift;
      guides.x.push(s.line);
    }
  } else if (edges.left) {
    const s = nearest([rect.x_mm], t.x, threshold);
    if (s) {
      out.x_mm += s.shift;
      out.w_mm -= s.shift;
      guides.x.push(s.line);
    }
  }
  if (edges.bottom) {
    const s = nearest([rect.y_mm + rect.h_mm], t.y, threshold);
    if (s) {
      out.h_mm += s.shift;
      guides.y.push(s.line);
    }
  } else if (edges.top) {
    const s = nearest([rect.y_mm], t.y, threshold);
    if (s) {
      out.y_mm += s.shift;
      out.h_mm -= s.shift;
      guides.y.push(s.line);
    }
  }
  return { rect: out, guides };
}

/**
 * Resizes `start` by a pointer delta on the given edges, keeping the
 * opposite edges fixed and every side at least `min`. `keepRatio` (corner
 * handles) scales both sides by the larger relative change.
 */
export function resizeRect(start: Rect, dx: number, dy: number, edges: Edges, min: number, keepRatio: boolean): Rect {
  let w = start.w_mm + (edges.right ? dx : edges.left ? -dx : 0);
  let h = start.h_mm + (edges.bottom ? dy : edges.top ? -dy : 0);
  const corner = (edges.left || edges.right) && (edges.top || edges.bottom);
  if (keepRatio && corner && start.w_mm > 0 && start.h_mm > 0) {
    const f = Math.max(w / start.w_mm, h / start.h_mm);
    w = start.w_mm * f;
    h = start.h_mm * f;
  }
  w = Math.max(min, w);
  h = Math.max(min, h);
  // The left edge can't go before the label start.
  if (edges.left) w = Math.min(w, start.x_mm + start.w_mm);
  return {
    x_mm: edges.left ? start.x_mm + start.w_mm - w : start.x_mm,
    y_mm: edges.top ? start.y_mm + start.h_mm - h : start.y_mm,
    w_mm: w,
    h_mm: h,
  };
}

/**
 * Rounds every field to 0.1 mm and keeps the box from starting before the
 * label (nothing can be printed there).
 */
export function roundRect(r: Rect): Rect {
  const q = (v: number) => Math.round(v * 10) / 10;
  return { x_mm: Math.max(0, q(r.x_mm)), y_mm: q(r.y_mm), w_mm: q(r.w_mm), h_mm: q(r.h_mm) };
}
