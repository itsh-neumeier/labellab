// Checks the UI language files against German (the reference):
// unknown keys and changed {placeholders} are errors, missing keys only a
// warning (those texts fall back to English, then German). Run by `npm run build`.
import { readdirSync, readFileSync } from "node:fs";

const dir = new URL("../src/i18n/", import.meta.url);
const load = (file) => JSON.parse(readFileSync(new URL(file, dir), "utf8"));
// `{{…}}` is literal text in examples, not a placeholder.
const placeholders = (s) => (s.match(/(?<!\{)\{\w+\}(?!\})/g) ?? []).sort().join(" ");

const de = load("de.json");
let errors = 0;
for (const file of readdirSync(dir).filter((f) => f.endsWith(".json") && f !== "de.json")) {
  const dict = load(file);
  const missing = Object.keys(de).filter((k) => !(k in dict));
  for (const [key, text] of Object.entries(dict)) {
    if (!(key in de)) {
      console.error(`${file}: unknown key ${key}`);
      errors++;
    } else if (placeholders(text) !== placeholders(de[key])) {
      console.error(`${file}: placeholders of ${key} differ from de.json`);
      errors++;
    }
  }
  if (missing.length) console.warn(`${file}: ${missing.length} untranslated (e.g. ${missing.slice(0, 3).join(", ")})`);
}
if (errors) process.exit(1);
