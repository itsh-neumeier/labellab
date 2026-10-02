// Typed wrappers around the Tauri commands in `src-tauri/src/lib.rs`.
// Shapes mirror `ll_core::label::Label` / `ll_core::device::Connection`
// (serde JSON), so `.llabel` files and IPC use the same format.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Symbology = "code128" | "ean13" | "ean8" | "upc_a" | "code39" | "itf";
export const SYMBOLOGIES: Symbology[] = ["code128", "ean13", "ean8", "upc_a", "code39", "itf"];
export const SYMBOLOGY_NAMES: Record<Symbology, string> = {
  code128: "Code 128",
  ean13: "EAN-13",
  ean8: "EAN-8",
  upc_a: "UPC-A",
  code39: "Code 39",
  itf: "ITF",
};

export type TextAlign = "left" | "center" | "right";

/** Free-layout box in mm: x along the tape, y across it from the top. */
export interface Rect {
  x_mm: number;
  y_mm: number;
  w_mm: number;
  h_mm: number;
}

export type Element =
  | {
      type: "text";
      text: string;
      size_pt?: number | null;
      align: TextAlign;
      /** System font family; empty/absent = default font. */
      font?: string | null;
      bold?: boolean;
      italic?: boolean;
    }
  | { type: "qr"; data: string }
  | { type: "barcode"; symbology: Symbology; data: string }
  | { type: "image"; path: string; invert: boolean; brightness?: number; contrast?: number }
  | { type: "symbol"; name: string; invert: boolean }
  | { type: "fill" }
  | { type: "shape"; shape: ShapeKind; stroke_mm: number; filled?: boolean };

export type ShapeKind = "line" | "rectangle" | "rounded_rectangle" | "ellipse";

/** An element plus its box and rotation (`ll_core::label::Item`, serialized flat). */
export type Item = Element & { rect?: Rect | null; rotation?: number; locked?: boolean };

/** Running number for `{{n}}`/`{{a}}`/`{{A}}`. */
export interface Numbering {
  start: number;
  step: number;
}

export type Layout =
  | { kind: "cable_flag"; text: string; diameter_mm: number; flag_mm: number }
  | { kind: "cable_wrap"; text: string; diameter_mm: number; repeats: number | null; vertical: boolean }
  | {
      kind: "patch_panel";
      count: number;
      pitch_mm: number;
      start: number;
      step: number;
      prefix: string;
      digits: number;
      separators: boolean;
      margin_mm: number;
    };

export type BorderStyle = "solid" | "dashed" | "dotted" | "double" | "striped";

export interface Border {
  style: BorderStyle;
  width_mm: number;
  sides: { top: boolean; bottom: boolean; left: boolean; right: boolean };
  pattern_mm: number;
  inset_mm: number;
}

export interface Label {
  version: number;
  elements: Item[];
  /** With `min_length_mm`: exactly that long, content beyond is cut off. */
  fixed_length?: boolean;
  gap_mm: number;
  padding_mm: number;
  min_length_mm?: number | null;
  /** Solid border (older files); `border` takes precedence. */
  frame: boolean;
  border?: Border | null;
  /** Stacked tape strips (multi-tape label), default 1. */
  strips?: number;
}

export type Connection =
  | { kind: "serial"; port: string; baud_rate: number }
  | { kind: "bluetooth"; device_id: string }
  | { kind: "usb"; spec: string | null };

export interface Tape {
  width_mm: number;
  /** Printable height across the tape in mm (the editor's vertical extent). */
  printable_mm: number;
}

export interface Model {
  name: string;
  tapes: Tape[];
}

export interface Device {
  name: string;
  connection: Connection;
  /** Recognized printer model (USB id or Bluetooth name), else null. */
  model: string | null;
}

export interface Status {
  width_mm: number;
  media_type: number;
  tape_color: number;
  text_color: number;
  tape_color_id: string | null;
  text_color_id: string | null;
  has_error: boolean;
  error1: number;
  error2: number;
}

export const models = () => invoke<Model[]>("models");
export const defaultMarginDots = () => invoke<number>("default_margin_dots");
export const listDevices = () => invoke<{ devices: Device[]; warnings: string[] }>("list_devices");
export const queryStatus = (connection: Connection) => invoke<Status>("query_status", { connection });
export const loadLabel = (path: string) => invoke<Label>("load_label", { path });
export const saveLabel = (path: string, label: Label) => invoke<void>("save_label", { path, label });

/** Base64 PNG of the label as it will be printed (same render path),
 * plus the indices of elements whose text does not fit its box. */
export const renderPreview = (
  label: Label,
  model: string,
  widthMm: number,
  row: number | null,
  numbering: Numbering | null,
  scale: number,
) => invoke<{ png: string; overflowing: number[] }>("render_preview", { label, model, widthMm, row, numbering, scale });

/** Same text in every language, or translated (`{"de": …, "en": …}`). */
export type I18nText = string | Record<string, string>;

export interface Icon {
  id: string;
  name: I18nText;
  category?: string | null;
  tags?: string[];
  svg: string;
  license?: string | null;
  author?: string | null;
  source?: string | null;
}

/** A `.llabel-iconset` (`ll_render::IconSet`) plus whether it ships with LabelLab. */
export interface IconSet {
  id: string;
  name: I18nText;
  description?: I18nText | null;
  license?: string | null;
  source?: string | null;
  categories: { id: string; name: I18nText }[];
  icons: Icon[];
  builtin: boolean;
}

export const iconsets = () => invoke<IconSet[]>("iconsets");
export const importIconset = (path: string) => invoke<IconSet>("import_iconset", { path });
export const removeIconset = (id: string) => invoke<void>("remove_iconset", { id });
export const generateLayout = (layout: Layout, model: string, widthMm: number) =>
  invoke<Label>("generate_layout", { layout, model, widthMm });

/** Boxes for every element as rendered (flow elements included). */
export const resolveRects = (label: Label, model: string, widthMm: number) =>
  invoke<Rect[]>("resolve_rects", { label, model, widthMm });

export interface PrintJob {
  copies: number;
  cut: boolean;
  chain: boolean;
  marginDots: number;
  /** 1-based inclusive record range of the loaded CSV; null = all. */
  rows: [number, number] | null;
  /** Without CSV: labels in a numbered series (null = single label). */
  count: number | null;
  numbering: Numbering | null;
}

export const printLabel = (args: { label: Label; connection: Connection; model: string; job: PrintJob }) =>
  invoke<void>("print_label", args);

export interface Progress {
  done: number;
  total: number;
}

export const onPrintProgress = (cb: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("print-progress", (e) => cb(e.payload));

export interface Csv {
  headers: string[];
  rows: string[][];
}

export const loadCsv = (path: string) => invoke<Csv>("load_csv", { path });
export const clearCsv = () => invoke<void>("clear_csv");
export const fontFamilies = () => invoke<string[]>("font_families");

export interface Pairable {
  id: string;
  name: string;
  model: string | null;
}

export const discoverBluetooth = () => invoke<Pairable[]>("discover_bluetooth");
export const pairBluetooth = (id: string) => invoke<void>("pair_bluetooth", { id });
