// Tape/ink color combinations for the preview. Color ids match
// `ll_protocol::media` (status bytes 24/25), so a status read can pick the
// combination of the loaded tape. Printing is always 1-bit; colors only
// affect how the preview looks.

/** Preview colors per tape color id. `null` = transparent tape. */
export const TAPE_CSS: Record<string, string | null> = {
  white: "#ffffff",
  clear: null,
  clear_white_text: null,
  red: "#d9372b",
  blue: "#2563c9",
  yellow: "#ffd31a",
  green: "#2e9e4f",
  black: "#151515",
  matte_white: "#f2f2ef",
  matte_clear: null,
  matte_silver: "#b9bcc0",
  satin_gold: "#c9a64b",
  satin_silver: "#c8cacc",
  blue_d: "#2f6fd6",
  red_d: "#e0453a",
  fluorescent_orange: "#ff7a1a",
  fluorescent_yellow: "#e9ff3a",
  berry_pink: "#d94c8a",
  light_gray: "#c9c9c9",
  lime_green: "#9bd13a",
  yellow_f: "#ffe04a",
  pink_f: "#f5a3c7",
  blue_f: "#7fb2f0",
  white_heat_shrink: "#ffffff",
  white_flex: "#ffffff",
  yellow_flex: "#ffd31a",
};

/** Preview colors per text (ink) color id. */
export const INK_CSS: Record<string, string> = {
  black: "#111111",
  white: "#ffffff",
  red: "#c62828",
  blue: "#1f4fbf",
  gold: "#b8892a",
  blue_f: "#1f4fbf",
};

export interface TapeStyle {
  tape: string;
  ink: string;
}

/** Common laminated tape combinations, grouped by ink color. */
export const TAPE_STYLES: TapeStyle[] = [
  ...[
    "white",
    "clear",
    "yellow",
    "red",
    "blue",
    "green",
    "satin_gold",
    "matte_silver",
    "matte_white",
    "matte_clear",
    "fluorescent_orange",
    "fluorescent_yellow",
  ].map((tape) => ({ tape, ink: "black" })),
  ...["clear", "black", "red", "blue", "green"].map((tape) => ({ tape, ink: "white" })),
  ...["white", "clear", "yellow"].map((tape) => ({ tape, ink: "red" })),
  ...["white", "clear", "yellow"].map((tape) => ({ tape, ink: "blue" })),
  ...["clear", "white", "black"].map((tape) => ({ tape, ink: "gold" })),
];

export const styleKey = (s: TapeStyle) => `${s.ink}_on_${s.tape}`;

export function parseStyleKey(key: string): TapeStyle | null {
  const m = /^(.+)_on_(.+)$/.exec(key);
  return m ? { ink: m[1], tape: m[2] } : null;
}
