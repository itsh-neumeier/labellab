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
  | { type: "image"; path: string; invert: boolean };

/** An element plus its box (`ll_core::label::Item`, serialized flat). */
export type Item = Element & { rect?: Rect | null };

export interface Label {
  version: number;
  elements: Item[];
  gap_mm: number;
  padding_mm: number;
  min_length_mm?: number | null;
  frame: boolean;
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

/** Base64 PNG of the label as it will be printed (same render path). */
export const renderPreview = (label: Label, model: string, widthMm: number, row: number | null) =>
  invoke<string>("render_preview", { label, model, widthMm, row });

/** Boxes for every element as rendered (flow elements included). */
export const resolveRects = (label: Label, model: string, widthMm: number) =>
  invoke<Rect[]>("resolve_rects", { label, model, widthMm });

export interface PrintJob {
  copies: number;
  preCut: boolean;
  postCut: boolean;
  marginDots: number;
  /** 1-based inclusive record range of the loaded CSV; null = all. */
  rows: [number, number] | null;
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
