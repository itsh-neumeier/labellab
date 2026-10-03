// Pipe marking per DIN 2403: substance groups with their colours. The
// printer prints one colour on the tape colour, so a group recommends a
// cassette (tape and ink colour, as a preview tape style when LabelLab has
// one) and how the additional colour in the arrow tips can be shown.

export type PipeDirection = "right" | "left" | "both";
export type TipFill = "none" | "solid" | "hatched";

export interface PipeGroup {
  id: number;
  /** i18n key of the group name. */
  name: string;
  /** Tape (substance) colour and ink (text/border) colour as i18n `color.*` ids. */
  tape: string;
  ink: string;
  /** Additional colour of the group (DIN), if any. */
  extra: "red" | "black" | null;
  /** Preview tape style key (`ink_on_tape`) when a matching cassette exists. */
  style: string | null;
  /** Tip fill that shows the additional colour as well as the ink allows. */
  tips: TipFill;
  /** Usual media of the group (suggestions for the text). */
  media: string[];
  /** Usual GHS pictograms (`ghs:GHS0x`). */
  symbols: string[];
}

export const PIPE_GROUPS: PipeGroup[] = [
  { id: 0, name: "pipe.group0", tape: "blue", ink: "white", extra: null, style: "white_on_blue", tips: "none",
    media: ["Sauerstoff"], symbols: ["ghs:GHS03", "ghs:GHS04"] },
  { id: 1, name: "pipe.group1", tape: "green", ink: "white", extra: null, style: "white_on_green", tips: "none",
    media: ["Wasser", "Trinkwasser", "Kaltwasser", "Warmwasser", "Zirkulation", "Heizung Vorlauf", "Heizung Rücklauf",
      "Kühlwasser", "Löschwasser", "Regenwasser", "Abwasser", "Solar Vorlauf", "Solar Rücklauf"], symbols: [] },
  { id: 2, name: "pipe.group2", tape: "red", ink: "white", extra: null, style: "white_on_red", tips: "none",
    media: ["Dampf", "Heißdampf", "Kondensat"], symbols: [] },
  { id: 3, name: "pipe.group3", tape: "gray", ink: "black", extra: null, style: null, tips: "none",
    media: ["Luft", "Druckluft", "Vakuum"], symbols: [] },
  // The red additional colour cannot be printed: tips left free (or hatched by choice).
  { id: 4, name: "pipe.group4", tape: "yellow", ink: "black", extra: "red", style: "black_on_yellow", tips: "none",
    media: ["Erdgas", "Propan", "Wasserstoff", "Acetylen", "Biogas"], symbols: ["ghs:GHS02", "ghs:GHS04"] },
  { id: 5, name: "pipe.group5", tape: "yellow", ink: "black", extra: "black", style: "black_on_yellow", tips: "solid",
    media: ["Stickstoff", "Kohlendioxid", "Argon", "Helium"], symbols: ["ghs:GHS04"] },
  { id: 6, name: "pipe.group6", tape: "orange", ink: "black", extra: null, style: "black_on_fluorescent_orange", tips: "none",
    media: ["Säuren", "Salzsäure", "Schwefelsäure"], symbols: ["ghs:GHS05"] },
  { id: 7, name: "pipe.group7", tape: "violet", ink: "white", extra: null, style: null, tips: "none",
    media: ["Laugen", "Natronlauge", "Kalilauge"], symbols: ["ghs:GHS05"] },
  { id: 8, name: "pipe.group8", tape: "brown", ink: "white", extra: "red", style: null, tips: "none",
    media: ["Heizöl", "Diesel", "Benzin"], symbols: ["ghs:GHS02"] },
  // Black additional colour on white ink cannot be printed either.
  { id: 9, name: "pipe.group9", tape: "brown", ink: "white", extra: "black", style: null, tips: "none",
    media: ["Hydrauliköl", "Kältemittel"], symbols: [] },
];

/** DIN 2403 colours (RAL) for the coloured wizard preview. */
export const PIPE_CSS: Record<string, string> = {
  blue: "#154889", // RAL 5005
  green: "#237f52", // RAL 6032
  red: "#9b2423", // RAL 3001
  gray: "#8f8f8f", // RAL 7004
  yellow: "#f9a800", // RAL 1003
  orange: "#d4652f", // RAL 2010
  violet: "#924e7d", // RAL 4008
  brown: "#79553d", // RAL 8002
  white: "#ffffff",
  black: "#111111",
};

/** Colours a pipe marker is shown in: its own, else its group's. */
export function pipeColors(m: {
  group?: number | null;
  colors?: { background?: string | null; ink?: string | null; extra?: string | null } | null;
}): { background: string; ink: string; extra: string | null } {
  const g = PIPE_GROUPS.find((x) => x.id === m.group);
  return {
    background: m.colors?.background ?? (g ? PIPE_CSS[g.tape] : PIPE_CSS.white),
    ink: m.colors?.ink ?? (g ? PIPE_CSS[g.ink] : PIPE_CSS.black),
    extra: m.colors?.extra ?? (g?.extra ? PIPE_CSS[g.extra] : null),
  };
}

export function pipeGroup(id: number | null | undefined): PipeGroup | undefined {
  return PIPE_GROUPS.find((g) => g.id === id);
}

/** Default label length in mm for a tape: about the DIN ratio (37 × 180 mm). */
export function pipeLength(tapeMm: number): number {
  return Math.round(tapeMm * 4.8);
}
