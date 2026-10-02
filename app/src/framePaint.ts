// Pixel editor ("small paint") for the three segments of a decorative
// frame. A drawing is one bit per print dot (ink or not) at the tape's
// printable height; it is stored in the frame as an SVG wrapping a PNG
// (`<image href="data:image/png;base64,…">`), which the frame renderer
// draws like any other segment. Segments can still be plain SVG code.

import * as api from "./api";
import { open } from "@tauri-apps/plugin-dialog";
import { openImageEditor } from "./imageEditor";
import { errorText, t } from "./i18n";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

export type Seg = "start" | "middle" | "end";
export const SEGS: Seg[] = ["start", "middle", "end"];

type Tool = "pencil" | "eraser" | "line" | "rect" | "ellipse" | "fill";

/** One bit per pixel, row-major: 1 = ink. */
interface Drawing {
  w: number;
  h: number;
  px: Uint8Array;
}

/** An inserted image being placed before it is stamped into the drawing. */
interface Floating {
  img: HTMLImageElement;
  x: number;
  y: number;
  scale: number;
  threshold: number;
  dither: boolean;
  bits: Drawing | null;
}

const MAX_UNDO = 40;
const MAX_WIDTH = 2000;
const MIN_ZOOM = 2;
const MAX_ZOOM = 14;
/** Display height the canvas aims for, in CSS pixels. */
const TARGET_DISPLAY_PX = 300;
const INK = "#111";
const GRID = "#e3e7ec";
const PREVIEW_INK = "rgba(26, 115, 232, 0.65)";

const drawings: Record<Seg, Drawing | null> = { start: null, middle: null, end: null };
/** SVG code of segments not edited as drawings. */
const svgCode: Record<Seg, string> = { start: "", middle: "", end: "" };
const undo: Record<Seg, Uint8Array[]> = { start: [], middle: [], end: [] };
let current: Seg = "start";
let tool: Tool = "pencil";
let brush = 1;
let filled = false;
let height = 70;
let floating: Floating | null = null;
let onChange: () => void = () => {};

// Stroke in progress.
let down: { x: number; y: number; value: number; last: [number, number] } | null = null;
let hover: [number, number] | null = null;

const blank = (w: number, h: number): Drawing => ({ w, h, px: new Uint8Array(w * h) });
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function setPx(d: Drawing, x: number, y: number, v: number): void {
  if (x >= 0 && y >= 0 && x < d.w && y < d.h) d.px[y * d.w + x] = v;
}

function getPx(d: Drawing, x: number, y: number): number {
  return x >= 0 && y >= 0 && x < d.w && y < d.h ? d.px[y * d.w + x] : 0;
}

/** Square brush of `brush` pixels centred on (x, y). */
function stamp(d: Drawing, x: number, y: number, v: number): void {
  const o = Math.floor((brush - 1) / 2);
  for (let dy = 0; dy < brush; dy++) for (let dx = 0; dx < brush; dx++) setPx(d, x - o + dx, y - o + dy, v);
}

function line(d: Drawing, x0: number, y0: number, x1: number, y1: number, v: number): void {
  const dx = Math.abs(x1 - x0);
  const dy = -Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    stamp(d, x0, y0, v);
    if (x0 === x1 && y0 === y1) break;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x0 += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y0 += sy;
    }
  }
}

function rect(d: Drawing, x0: number, y0: number, x1: number, y1: number, v: number): void {
  const [ax, bx] = [Math.min(x0, x1), Math.max(x0, x1)];
  const [ay, by] = [Math.min(y0, y1), Math.max(y0, y1)];
  for (let y = ay; y <= by; y++) {
    for (let x = ax; x <= bx; x++) {
      const edge = x - ax < brush || bx - x < brush || y - ay < brush || by - y < brush;
      if (filled || edge) setPx(d, x, y, v);
    }
  }
}

function ellipse(d: Drawing, x0: number, y0: number, x1: number, y1: number, v: number): void {
  const [ax, bx] = [Math.min(x0, x1), Math.max(x0, x1)];
  const [ay, by] = [Math.min(y0, y1), Math.max(y0, y1)];
  const cx = (ax + bx) / 2;
  const cy = (ay + by) / 2;
  const rx = (bx - ax) / 2 + 0.5;
  const ry = (by - ay) / 2 + 0.5;
  const inside = (x: number, y: number, shrink: number) => {
    const a = rx - shrink;
    const b = ry - shrink;
    if (a <= 0 || b <= 0) return false;
    return ((x - cx) / a) ** 2 + ((y - cy) / b) ** 2 <= 1;
  };
  for (let y = ay; y <= by; y++) {
    for (let x = ax; x <= bx; x++) {
      if (inside(x, y, 0) && (filled || !inside(x, y, brush))) setPx(d, x, y, v);
    }
  }
}

/** 4-connected flood fill of the area at (x, y) with `v`. */
function flood(d: Drawing, x: number, y: number, v: number): void {
  const from = getPx(d, x, y);
  if (from === v) return;
  const stack: [number, number][] = [[x, y]];
  while (stack.length) {
    const [px, py] = stack.pop()!;
    if (px < 0 || py < 0 || px >= d.w || py >= d.h || d.px[py * d.w + px] !== from) continue;
    d.px[py * d.w + px] = v;
    stack.push([px + 1, py], [px - 1, py], [px, py + 1], [px, py - 1]);
  }
}

function mirrored(d: Drawing): Drawing {
  const out = blank(d.w, d.h);
  for (let y = 0; y < d.h; y++) for (let x = 0; x < d.w; x++) out.px[y * d.w + (d.w - 1 - x)] = d.px[y * d.w + x];
  return out;
}

/** Nearest-neighbour scale to height `h` (keeps the aspect ratio). */
function scaledTo(d: Drawing, h: number): Drawing {
  const w = Math.max(1, Math.round((d.w * h) / d.h));
  const out = blank(w, h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) out.px[y * w + x] = getPx(d, Math.floor((x * d.w) / w), Math.floor((y * d.h) / h));
  }
  return out;
}

function resized(d: Drawing, w: number): Drawing {
  const out = blank(w, d.h);
  for (let y = 0; y < d.h; y++) for (let x = 0; x < Math.min(w, d.w); x++) out.px[y * w + x] = d.px[y * d.w + x];
  return out;
}

/** Simple default frame: double lines with closed ends, `h` dots high. */
function templateDrawings(h: number): Record<Seg, Drawing> {
  const t = Math.max(2, Math.round(h / 24));
  const inset = Math.max(1, Math.round(h / 35));
  const middle = blank(16, h);
  const start = blank(Math.max(8, t * 3), h);
  for (const d of [middle, start]) {
    for (let x = 0; x < d.w; x++) {
      for (let k = 0; k < t; k++) {
        setPx(d, x, inset + k, 1);
        setPx(d, x, h - 1 - inset - k, 1);
      }
    }
  }
  for (let y = inset; y < h - inset; y++) for (let k = 0; k < t; k++) setPx(start, inset + k, y, 1);
  return { start, middle, end: mirrored(start) };
}

// ---------------------------------------------------------------- PNG/SVG

function toCanvas(d: Drawing, color = "#000"): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = d.w;
  c.height = d.h;
  const ctx = c.getContext("2d")!;
  const img = ctx.createImageData(d.w, d.h);
  const rgb = color === "#000" ? [0, 0, 0] : [17, 17, 17];
  for (let i = 0; i < d.px.length; i++) {
    if (!d.px[i]) continue;
    img.data.set([...rgb, 255], i * 4);
  }
  ctx.putImageData(img, 0, 0);
  return c;
}

/** The drawing as an SVG segment (PNG embedded as data URL). */
function drawingSvg(d: Drawing): string {
  const href = toCanvas(d).toDataURL("image/png");
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${d.w}" height="${d.h}" viewBox="0 0 ${d.w} ${d.h}">` +
    `<image width="${d.w}" height="${d.h}" image-rendering="optimizeSpeed" href="${href}"/></svg>`
  );
}

/** Ink from an image: dark, opaque pixels (optionally Floyd–Steinberg dithered). */
function bitsFrom(source: CanvasImageSource, w: number, h: number, threshold = 128, dither = false): Drawing {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d", { willReadFrequently: true })!;
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(source, 0, 0, w, h);
  const data = ctx.getImageData(0, 0, w, h).data;
  const lum = new Float32Array(w * h);
  for (let i = 0; i < w * h; i++) {
    const a = data[i * 4 + 3] / 255;
    const l = 0.299 * data[i * 4] + 0.587 * data[i * 4 + 1] + 0.114 * data[i * 4 + 2];
    lum[i] = 255 - a * (255 - l); // transparent = white
  }
  const out = blank(w, h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const ink = lum[i] < threshold;
      out.px[i] = ink ? 1 : 0;
      if (!dither) continue;
      const err = lum[i] - (ink ? 0 : 255);
      if (x + 1 < w) lum[i + 1] += (err * 7) / 16;
      if (y + 1 < h) {
        if (x > 0) lum[i + w - 1] += (err * 3) / 16;
        lum[i + w] += (err * 5) / 16;
        if (x + 1 < w) lum[i + w + 1] += err / 16;
      }
    }
  }
  return out;
}

async function loadImage(src: string): Promise<HTMLImageElement> {
  const img = new Image();
  img.src = src;
  await img.decode();
  return img;
}

/** A segment made by this editor (one embedded PNG) back as a drawing. */
async function parseDrawing(svg: string): Promise<Drawing | null> {
  const images = svg.match(/<image\b/g)?.length ?? 0;
  const href = /(?:xlink:)?href="(data:image\/png;base64,[^"]+)"/.exec(svg)?.[1];
  if (images !== 1 || !href || /<(path|rect|circle|ellipse|line|polyline|polygon|text)\b/.test(svg)) return null;
  const img = await loadImage(href);
  return bitsFrom(img, img.naturalWidth, img.naturalHeight);
}

/** Renders SVG code into a drawing of height `h` (for "edit as drawing"). */
async function rasterizeSvg(svg: string, h: number): Promise<Drawing> {
  const img = await loadImage(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`);
  const w = Math.max(1, Math.round((img.naturalWidth * h) / Math.max(1, img.naturalHeight)));
  return bitsFrom(img, Math.min(w, MAX_WIDTH), h);
}

// ---------------------------------------------------------------- state API

/** SVG code of a segment (drawing → wrapped PNG). */
export function segmentSvg(seg: Seg): string {
  const d = drawings[seg];
  return d ? drawingSvg(d) : svgCode[seg];
}

/** New frame: template drawings at `h` dots. */
export function newPaint(h: number): void {
  height = h;
  floating = null;
  const tpl = templateDrawings(h);
  for (const s of SEGS) {
    drawings[s] = tpl[s];
    svgCode[s] = "";
    undo[s] = [];
  }
  select("start");
}

/** Existing frame: drawings made here open as drawings, other SVG as code. */
export async function loadPaint(frame: Record<Seg, string>, h: number): Promise<void> {
  height = h;
  floating = null;
  for (const s of SEGS) {
    svgCode[s] = frame[s];
    undo[s] = [];
    let d: Drawing | null = null;
    try {
      d = await parseDrawing(frame[s]);
    } catch {
      d = null;
    }
    drawings[s] = d && d.h !== h ? scaledTo(d, h) : d;
  }
  // Keep all drawings at one height (their proportions matter).
  const first = SEGS.map((s) => drawings[s]).find(Boolean);
  if (first) height = first.h;
  select("start");
}

// ---------------------------------------------------------------- UI

function zoom(): number {
  return clamp(Math.floor(TARGET_DISPLAY_PX / height), MIN_ZOOM, MAX_ZOOM);
}

function select(seg: Seg): void {
  current = seg;
  floating = null;
  document.querySelectorAll<HTMLButtonElement>("#fe-tabs button").forEach((b) => b.classList.toggle("on", b.dataset.seg === seg));
  const d = drawings[seg];
  $("fe-paint").hidden = !d;
  $("fe-code").hidden = !!d;
  $<HTMLButtonElement>("fe-mode-draw").classList.toggle("on", !!d);
  $<HTMLButtonElement>("fe-mode-code").classList.toggle("on", !d);
  $<HTMLTextAreaElement>("fe-svg").value = svgCode[seg];
  $("fe-from-start").hidden = seg !== "end";
  $("fe-place").hidden = true;
  if (d) $<HTMLInputElement>("fe-width").value = String(d.w);
  $<HTMLInputElement>("fe-height").value = String(height);
  draw();
}

function changed(): void {
  draw();
  onChange();
}

function pushUndo(): void {
  const d = drawings[current];
  if (!d) return;
  const stack = undo[current];
  stack.push(new Uint8Array(d.px));
  if (stack.length > MAX_UNDO) stack.shift();
}

function floatingBits(f: Floating): Drawing {
  if (!f.bits) {
    const w = Math.max(1, Math.round(f.img.naturalWidth * f.scale));
    const h = Math.max(1, Math.round(f.img.naturalHeight * f.scale));
    f.bits = bitsFrom(f.img, Math.min(w, MAX_WIDTH), Math.min(h, 4 * height), f.threshold, f.dither);
  }
  return f.bits;
}

/** Widens the drawing so the floating image fits (inserted images set the width). */
function fitFloating(): void {
  const d = drawings[current];
  if (!floating || !d) return;
  const need = Math.min(MAX_WIDTH, floating.x + floatingBits(floating).w);
  if (need > d.w) {
    drawings[current] = resized(d, need);
    $<HTMLInputElement>("fe-width").value = String(need);
  }
}

function shapePreview(d: Drawing): Drawing | null {
  if (!down || !hover || !["line", "rect", "ellipse"].includes(tool)) return null;
  const p = blank(d.w, d.h);
  const [x1, y1] = hover;
  if (tool === "line") line(p, down.x, down.y, x1, y1, 1);
  else if (tool === "rect") rect(p, down.x, down.y, x1, y1, 1);
  else ellipse(p, down.x, down.y, x1, y1, 1);
  return p;
}

function draw(): void {
  const d = drawings[current];
  const canvas = $<HTMLCanvasElement>("fe-canvas");
  if (!d) return;
  const z = zoom();
  canvas.width = d.w * z;
  canvas.height = d.h * z;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  if (z >= 5) {
    ctx.fillStyle = GRID;
    for (let x = 1; x < d.w; x++) ctx.fillRect(x * z, 0, 1, canvas.height);
    for (let y = 1; y < d.h; y++) ctx.fillRect(0, y * z, canvas.width, 1);
  }
  const paint = (src: Drawing, color: string, ox = 0, oy = 0) => {
    ctx.fillStyle = color;
    for (let y = 0; y < src.h; y++) {
      for (let x = 0; x < src.w; x++) {
        if (src.px[y * src.w + x]) ctx.fillRect((x + ox) * z, (y + oy) * z, z, z);
      }
    }
  };
  paint(d, INK);
  const preview = shapePreview(d);
  if (preview) paint(preview, down?.value ? PREVIEW_INK : "rgba(255,255,255,0.85)");
  if (floating) paint(floatingBits(floating), PREVIEW_INK, floating.x, floating.y);
}

function cell(e: PointerEvent): [number, number] {
  const canvas = $<HTMLCanvasElement>("fe-canvas");
  const r = canvas.getBoundingClientRect();
  const z = zoom();
  const sx = canvas.width / r.width;
  return [Math.floor(((e.clientX - r.left) * sx) / z), Math.floor(((e.clientY - r.top) * sx) / z)];
}

function bindCanvas(): void {
  const canvas = $<HTMLCanvasElement>("fe-canvas");
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());
  let drag: [number, number] | null = null;
  canvas.addEventListener("pointerdown", (e) => {
    const d = drawings[current];
    if (!d) return;
    e.preventDefault();
    canvas.setPointerCapture(e.pointerId);
    const [x, y] = cell(e);
    if (floating) {
      drag = [x - floating.x, y - floating.y];
      return;
    }
    // Right button (or the eraser) clears.
    const value = e.button === 2 || tool === "eraser" ? 0 : 1;
    pushUndo();
    if (tool === "fill") {
      flood(d, x, y, value);
      changed();
      return;
    }
    down = { x, y, value, last: [x, y] };
    hover = [x, y];
    if (tool === "pencil" || tool === "eraser") {
      stamp(d, x, y, value);
      draw();
    }
  });
  canvas.addEventListener("pointermove", (e) => {
    const d = drawings[current];
    if (!d) return;
    const [x, y] = cell(e);
    if (floating && drag) {
      floating.x = x - drag[0];
      floating.y = y - drag[1];
      draw();
      return;
    }
    if (!down) return;
    hover = [x, y];
    if (tool === "pencil" || tool === "eraser") {
      line(d, down.last[0], down.last[1], x, y, down.value);
      down.last = [x, y];
    }
    draw();
  });
  const finish = () => {
    drag = null;
    const d = drawings[current];
    if (!down || !d) return;
    const [x1, y1] = hover ?? [down.x, down.y];
    if (tool === "line") line(d, down.x, down.y, x1, y1, down.value);
    else if (tool === "rect") rect(d, down.x, down.y, x1, y1, down.value);
    else if (tool === "ellipse") ellipse(d, down.x, down.y, x1, y1, down.value);
    down = null;
    hover = null;
    changed();
  };
  canvas.addEventListener("pointerup", finish);
  canvas.addEventListener("pointercancel", finish);
}

function message(text: string): void {
  $("fe-msg").textContent = text;
}

/** Image from a file → image editor (crop, background) → placement. */
async function insertImage(path: string): Promise<void> {
  const preview = async (e: api.ImageEdit) => (await api.imageEditorSource(path, e)).masked;
  const result = await openImageEditor(path, {}, preview);
  if (result === null) return;
  const source = await api.imageEditorSource(path, result);
  const img = await loadImage(`data:image/png;base64,${source.masked}`);
  const scale = height / Math.max(1, img.naturalHeight);
  floating = { img, x: 0, y: 0, scale, threshold: 128, dither: false, bits: null };
  $<HTMLInputElement>("fe-place-scale").value = "100";
  $<HTMLInputElement>("fe-place-threshold").value = "128";
  $<HTMLInputElement>("fe-place-dither").checked = false;
  $("fe-place").hidden = false;
  pushUndo();
  fitFloating();
  draw();
}

function bindTools(): void {
  document.querySelectorAll<HTMLButtonElement>("#fe-tabs button").forEach((b) =>
    b.addEventListener("click", () => select(b.dataset.seg as Seg)),
  );
  const tools = document.querySelectorAll<HTMLButtonElement>("#fe-tools [data-tool]");
  tools.forEach((b) =>
    b.addEventListener("click", () => {
      tool = b.dataset.tool as Tool;
      tools.forEach((o) => o.classList.toggle("on", o === b));
    }),
  );
  $<HTMLSelectElement>("fe-brush").addEventListener("change", (e) => (brush = Number((e.target as HTMLSelectElement).value) || 1));
  $<HTMLInputElement>("fe-filled").addEventListener("change", (e) => (filled = (e.target as HTMLInputElement).checked));
  $("fe-undo").addEventListener("click", () => {
    const d = drawings[current];
    const prev = undo[current].pop();
    if (!d || !prev) return;
    // Undo of a width change restores the old width too.
    if (prev.length !== d.px.length) d.w = prev.length / d.h;
    d.px = prev;
    $<HTMLInputElement>("fe-width").value = String(d.w);
    changed();
  });
  $("fe-clear").addEventListener("click", () => {
    const d = drawings[current];
    if (!d) return;
    pushUndo();
    d.px.fill(0);
    changed();
  });
  $("fe-mirror").addEventListener("click", () => {
    const d = drawings[current];
    if (!d) return;
    pushUndo();
    d.px = mirrored(d).px;
    changed();
  });
  $("fe-from-start").addEventListener("click", () => {
    const s = drawings.start;
    if (!s) return;
    pushUndo();
    drawings.end = mirrored(s);
    select("end");
    onChange();
  });
  $("fe-width").addEventListener("change", () => {
    const d = drawings[current];
    if (!d) return;
    const w = clamp(Math.round(Number($<HTMLInputElement>("fe-width").value) || d.w), 1, MAX_WIDTH);
    pushUndo();
    drawings[current] = resized(d, w);
    changed();
  });
  $("fe-crop").addEventListener("click", () => {
    const d = drawings[current];
    if (!d) return;
    let [min, max] = [d.w, -1];
    for (let y = 0; y < d.h; y++) {
      for (let x = 0; x < d.w; x++) {
        if (!d.px[y * d.w + x]) continue;
        min = Math.min(min, x);
        max = Math.max(max, x);
      }
    }
    if (max < 0) return;
    pushUndo();
    const out = blank(max - min + 1, d.h);
    for (let y = 0; y < d.h; y++) for (let x = min; x <= max; x++) out.px[y * out.w + x - min] = d.px[y * d.w + x];
    drawings[current] = out;
    $<HTMLInputElement>("fe-width").value = String(out.w);
    changed();
  });
  $("fe-height").addEventListener("change", () => {
    const h = clamp(Math.round(Number($<HTMLInputElement>("fe-height").value) || height), 8, 512);
    height = h;
    for (const s of SEGS) {
      const d = drawings[s];
      if (d && d.h !== h) {
        drawings[s] = scaledTo(d, h);
        undo[s] = [];
      }
    }
    select(current);
    onChange();
  });
  $("fe-insert").addEventListener("click", async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: t("file.imageFilter"), extensions: ["png", "jpg", "jpeg", "bmp", "svg"] }],
    });
    if (typeof path !== "string") return;
    try {
      await insertImage(path);
    } catch (e) {
      message(t("error.prefix", { error: errorText(e) }));
    }
  });
  $("fe-paste").addEventListener("click", async () => {
    try {
      const pasted = await api.pasteClipboardImage();
      if (!pasted) {
        message(t("paint.noClipboardImage"));
        return;
      }
      await insertImage(pasted.path);
    } catch (e) {
      message(t("error.prefix", { error: errorText(e) }));
    }
  });
  const placeInput = () => {
    if (!floating) return;
    const base = height / Math.max(1, floating.img.naturalHeight);
    floating.scale = (base * Number($<HTMLInputElement>("fe-place-scale").value)) / 100;
    floating.threshold = Number($<HTMLInputElement>("fe-place-threshold").value);
    floating.dither = $<HTMLInputElement>("fe-place-dither").checked;
    floating.bits = null;
    fitFloating();
    draw();
  };
  for (const id of ["fe-place-scale", "fe-place-threshold", "fe-place-dither"]) $(id).addEventListener("input", placeInput);
  $("fe-place-ok").addEventListener("click", () => {
    const d = drawings[current];
    if (!floating || !d) return;
    const bits = floatingBits(floating);
    for (let y = 0; y < bits.h; y++) {
      for (let x = 0; x < bits.w; x++) if (bits.px[y * bits.w + x]) setPx(d, x + floating.x, y + floating.y, 1);
    }
    floating = null;
    $("fe-place").hidden = true;
    changed();
  });
  $("fe-place-cancel").addEventListener("click", () => {
    floating = null;
    $("fe-place").hidden = true;
    // Back to the width before the image was inserted.
    $("fe-undo").click();
  });
  // Mode: drawing or SVG code.
  $("fe-mode-draw").addEventListener("click", async () => {
    if (drawings[current]) return;
    try {
      drawings[current] = await rasterizeSvg(svgCode[current], height);
    } catch {
      drawings[current] = blank(16, height);
    }
    undo[current] = [];
    select(current);
    onChange();
  });
  $("fe-mode-code").addEventListener("click", () => {
    const d = drawings[current];
    if (!d) return;
    svgCode[current] = drawingSvg(d);
    drawings[current] = null;
    select(current);
    onChange();
  });
  $<HTMLTextAreaElement>("fe-svg").addEventListener("input", (e) => {
    svgCode[current] = (e.target as HTMLTextAreaElement).value;
    onChange();
  });
  $("fe-load-svg").addEventListener("click", async () => {
    const path = await open({ multiple: false, filters: [{ name: "SVG", extensions: ["svg"] }] });
    if (typeof path !== "string") return;
    try {
      svgCode[current] = await api.readSvg(path);
      $<HTMLTextAreaElement>("fe-svg").value = svgCode[current];
      onChange();
    } catch (e) {
      message(t("error.prefix", { error: errorText(e) }));
    }
  });
}

/** Binds the editor once; `change` runs after every edit (preview). */
export function bindPaint(change: () => void): void {
  onChange = change;
  bindCanvas();
  bindTools();
}
