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
      /** Multiple of the normal line height (0.5–3); absent = 1. */
      line_spacing?: number | null;
    }
  | { type: "qr"; data: string }
  | { type: "barcode"; symbology: Symbology; data: string }
  | { type: "image"; path: string; invert: boolean; brightness?: number; contrast?: number; edit?: ImageEdit }
  | { type: "symbol"; name: string; invert: boolean }
  | { type: "fill" }
  | { type: "shape"; shape: ShapeKind; stroke_mm: number; filled?: boolean }
  | {
      type: "fuse_box";
      fields: FuseField[];
      /** Width of one ratio unit (module) in mm. */
      pitch_mm: number;
      separator?: FuseSeparator;
      vertical?: boolean;
      reverse?: boolean;
      size_pt?: number | null;
      align?: TextAlign;
      /** Multiple of the normal line height (0.5–3); absent = 1. */
      line_spacing?: number | null;
      font?: string | null;
      bold?: boolean;
      italic?: boolean;
    };

/** One field of a fuse box (`ll_core::fusebox::FuseField`). */
export interface FuseField {
  text: string;
  /** Relative width (1 = one module); absent = 1. */
  ratio?: number;
  /** Text direction override; absent = the element's `vertical`. */
  vertical?: boolean | null;
}

export type FuseSeparator = "marks" | "dashed" | "line" | "bold" | "frame" | "none";
export const FUSE_SEPARATORS: FuseSeparator[] = ["frame", "line", "bold", "dashed", "marks", "none"];

export type ShapeKind = "line" | "rectangle" | "rounded_rectangle" | "ellipse";

/** An element plus its box and rotation (`ll_core::label::Item`, serialized flat). */
export type VAlign = "top" | "middle" | "bottom";

/**
 * `halign`: content position in the box for codes/images/symbols (text uses
 * its own `align`); `valign`: for all elements. Absent = centered.
 */
export type Item = Element & {
  rect?: Rect | null;
  rotation?: number;
  locked?: boolean;
  halign?: TextAlign | null;
  valign?: VAlign | null;
  /** Not rendered or printed (layer list eye). */
  hidden?: boolean;
  /** User-given name in the layer list (stored as `title`: `name` is the symbol icon id). */
  title?: string | null;
};

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
    }
  | { kind: "single_flag"; text: string; diameter_mm: number; flag_mm: number }
  | ({ kind: "terminal_block"; rows: number } & FieldSpec)
  | ({
      kind: "fuse_box";
      vertical: boolean;
      main_switch: string;
      main_switch_mm: number;
      main_switch_right: boolean;
      /** Modules per field (merged devices); empty = one module per field. */
      spans?: number[];
      /** Text per field; empty = number. */
      texts?: string[];
    } & FieldSpec);

/** Numbered fields in a fixed pitch (terminal block, fuse box). */
export interface FieldSpec {
  count: number;
  pitch_mm: number;
  start: number;
  step: number;
  prefix: string;
  digits: number;
  separators: boolean;
  margin_mm: number;
}

export type BorderStyle = "solid" | "dashed" | "dotted" | "double" | "striped";

export interface Border {
  style: BorderStyle;
  width_mm: number;
  sides: { top: boolean; bottom: boolean; left: boolean; right: boolean };
  pattern_mm: number;
  inset_mm: number;
  /** Distance per side (overrides `inset_mm`); left/right count from the label margins. */
  insets_mm?: { top: number; bottom: number; left: number; right: number } | null;
}

export type Orientation = "landscape" | "portrait";

export interface Label {
  version: number;
  elements: Item[];
  /** Template the label was generated from (editable again). */
  source?: Layout | null;
  /** Decorative segment frame `set:frame`. */
  decor?: string | null;
  /** Editor orientation; portrait boxes are in portrait coordinates (absent = landscape). */
  orientation?: Orientation;
  /** With `min_length_mm`: exactly that long, content beyond is cut off. */
  fixed_length?: boolean;
  gap_mm: number;
  /** Right margin; also the left one unless `padding_start_mm` is set. */
  padding_mm: number;
  /** Left margin; absent = `padding_mm`. */
  padding_start_mm?: number | null;
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
/** One sheet of a `.llabel` document (`ll_core::document::Sheet`). */
export interface Sheet {
  name: string;
  width_mm?: number | null;
  label: Label;
}

export interface LabelDocument {
  version: number;
  sheets: Sheet[];
}

export const loadDocument = (path: string) => invoke<LabelDocument>("load_document", { path });
export const saveDocument = (path: string, document: LabelDocument) =>
  invoke<void>("save_document", { path, document });

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
  /** With chain + cut: also cut after every n-th label (0 = only at the end). */
  cutEvery: number;
  cutMarks: boolean;
  mirror: boolean;
  /** 1-based inclusive record range of the loaded CSV; null = all. */
  rows: [number, number] | null;
  /** Without CSV: labels in a numbered series (null = single label). */
  count: number | null;
  numbering: Numbering | null;
}

export const printLabel = (args: { label: Label; connection: Connection; model: string; job: PrintJob }) =>
  invoke<void>("print_label", args);

/** Feeds and cuts the tape without printing. */
export const feedCut = (args: { connection: Connection; model: string }) => invoke<void>("feed_cut", args);

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

/** A printed label in the history (`ll_core::history::Entry`) plus its preview. */
export interface HistoryEntry {
  id: string;
  printed_at: string;
  name: string;
  model: string;
  width_mm: number;
  count: number;
  /** Base64 PNG mask, empty if none. */
  preview: string;
}

export const history = () => invoke<HistoryEntry[]>("history");
export const recordHistory = (label: Label, model: string, widthMm: number, name: string, count: number) =>
  invoke<void>("record_history", { label, model, widthMm, name, count });
export const loadHistory = (id: string) => invoke<Label>("load_history", { id });

/** Non-destructive image edits (`ll_render::ImageEdit`). Crop in fractions of the rotated image. */
export interface ImageEdit {
  rotation?: number;
  flip_h?: boolean;
  flip_v?: boolean;
  crop?: { x: number; y: number; w: number; h: number } | null;
  background?: { color?: [number, number, number] | null; tolerance: number; contiguous: boolean } | null;
  halftone?: "dither" | "threshold" | null;
  threshold?: number | null;
}

export const imageEditorSource = (path: string, edit: ImageEdit) =>
  invoke<{ masked: string; plain: string; width: number; height: number }>("image_editor_source", { path, edit });

/** Stores a pasted image (base64) in the data folder; returns the file path. */
export const savePastedImage = (data: string, extension: string) =>
  invoke<string>("save_pasted_image", { data, extension });

/** Image from the system clipboard, stored as PNG; `null` without an image. */
export const pasteClipboardImage = () =>
  invoke<{ path: string; width: number; height: number } | null>("paste_clipboard_image");

/** Decoded status block for the printer info dialog (`printer_info`). */
export interface PrinterInfo {
  model: string | null;
  series_byte: number;
  model_byte: number;
  width_mm: number;
  media_type: number;
  media_type_id: string | null;
  tape_color_id: string | null;
  text_color_id: string | null;
  errors: string[];
  error1: number;
  error2: number;
  status_type: number;
  phase: number;
  notification: number;
  raw: number[];
}

export const printerInfo = (connection: Connection) => invoke<PrinterInfo>("printer_info", { connection });

/** A decorative segment frame (`ll_render::decor::FrameDef`); SVG strings. */
export interface FrameDef {
  id: string;
  name: I18nText;
  start: string;
  middle: string;
  end: string;
}

export interface FrameSet {
  id: string;
  name: I18nText;
  builtin: boolean;
  frames: (FrameDef & { preview: string })[];
}

export const frameSets = () => invoke<FrameSet[]>("frame_sets");
export const framePreview = (frame: FrameDef) => invoke<string>("frame_preview", { frame });
/** Saves into the user's set; returns the `set:frame` name. */
export const saveFrame = (frame: FrameDef) => invoke<string>("save_frame", { frame });
export const deleteFrame = (id: string) => invoke<void>("delete_frame", { id });
export const importFrameSet = (path: string) => invoke<string>("import_frame_set", { path });
export const removeFrameSet = (id: string) => invoke<void>("remove_frame_set", { id });
export const readSvg = (path: string) => invoke<string>("read_svg", { path });
export const saveTextFile = (path: string, content: string) => invoke<void>("save_text_file", { path, content });
