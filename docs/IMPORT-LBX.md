# Import von `.lbx`-Dateien (Hersteller-Editor)

Stand: 2026-10-02. Grundlage: zwei Beispieldateien des Nutzers (nicht im Repo, enthalten
Kundendaten/Bilder). Keine Hersteller-Dokumente verwendet; alles aus dem Dateiaufbau abgeleitet,
daher **unverifiziert** gegenüber einer Spezifikation.

## Dateiaufbau
- ZIP-Archiv mit `label.xml` (Inhalt), `prop.xml` (Metadaten: Programmversion, Datum) und
  eingebetteten Bildern (`Object0.jpg`, …, auch riesige `.bmp`).
- XML-Namensräume `pt`, `style`, `text`, `draw`, `image`, `barcode`, `table`, … unter
  `http://schemas.brother.info/ptouch/2007/lbx/…`.
- `pt:body/@currentSheet`, darunter je Blatt `style:sheet name="…"` → **Arbeitsblätter**.
- Längen immer in `pt` (1 pt = 0,3528 mm).

## Blatt (`style:paper`, `style:backGround`)
- `orientation="landscape"`: `width` = Bandbreite (68 pt ≈ 24 mm), `height` = Labellänge
  (283,6 pt ≈ 100 mm). `autoLength="true"` → Länge automatisch (dann `height` = Maximum).
- `marginTop/Bottom` (5,6 pt ≈ 2 mm) = Rand am Anfang/Ende (Länge),
  `marginLeft/Right` (8,4 pt ≈ 3 mm) = nicht bedruckbarer Bandrand (24 mm − 18,06 mm).
- `style:backGround x y width height` = bedruckbarer Bereich; `y` = Bandrand.
- Umrechnung zu LabelLab: `x_mm = x_pt·0,3528` (ab Schnitt), `y_mm = (y_pt − backGround.y)·0,3528`
  (ab bedruckbarem Bereich). Ränder → `padding_start_mm`/`padding_mm`, feste Länge →
  `min_length_mm` + `fixed_length` (bei `autoLength="false"`).
- `paperColor`/`paperInk` → Bandstil (nur Anzeige).

## Objekte (`pt:objects`), gemeinsam `pt:objectStyle x y width height angle`
| Objekt | Inhalt | Abbildung in LabelLab |
|---|---|---|
| `text:text` | `pt:data`, `text:logFont name/weight/italic`, `text:fontExt size`, `text:textAlign horizontalAlignment/verticalAlignment`, `text:stringItem charLen` + eigene Schrift je Abschnitt | Text: Schrift, Größe (pt), fett (weight ≥ 600), kursiv, `align`/`valign`; Abschnitte mit anderem Gewicht → `**…**`/`__…__` |
| `image:image` | `image:imageStyle fileName` (Datei im ZIP), `originalName`, `image:effect brightness/contrast` (50 = neutral), `image:mono operationKind/threshold`, `image:transparent`, `image:trimming` | Bild (Datei in den Datenordner entpacken), Helligkeit/Kontrast (−100..100 = (v−50)·2), Schwelle/Raster, Hintergrund entfernen (`transparent flag`), Zuschnitt |
| `barcode:barcode` | `barcode:barcodeStyle protocol` (`CODE39`, `EAN13`, `QRCODE`, `AZTECCODE`, …), `humanReadable`, `pt:data` | Barcode/QR; Code 39, EAN-13 (und Code 128, EAN-8, UPC-A, ITF) vorhanden; **Aztec fehlt** → Hinweis/Platzhalter |
| `text:datetime` | `dateTimeStyle mode="DATE"/"TIME"`, `format`, `atPrint="true"` | Text `{{datum}}` / `{{zeit}}` |
| `table:table` | `tableStyle row/column`, `gridPosition x/y` (Linienpositionen), `table:cell` | Rechteck + Linien (Form-Elemente) als Näherung; Zelltexte → Textelemente |
| `draw:frame` | `frameStyle category="SPECIAL" style="0"` – Rahmen aus der Hersteller-Bibliothek | Kein Nachbau der Grafik (Hersteller-Material) → einfacher/eigener Rahmenstil + Hinweis |
| `image:clipart` | `clipartStyle originalName="FONT,<Schriftname>,<Zeichen>,…"` – Symbol aus einer Hersteller-Schrift | Nicht übertragbar → Platzhalter-Symbol + Hinweis |

## Umsetzung (2026-10-02, ADR-038)
- `ll_core::lbx::import(path)` → `LbxImport { document, warnings }`; Bilder landen in
  `llappdata/imported/<Dateiname>/` (BMP als PNG, gleiche Datei nur einmal).
- Oberfläche: „Öffnen …“ nimmt `.llabel` und `.lbx`. Eine importierte Datei ist ein neues,
  ungespeichertes Dokument; Speichern schlägt `<Name>.llabel` vor. Hinweise erscheinen als Liste.
- CLI: `labellab import-lbx datei.lbx [-o ziel.llabel]`.
- Das zuletzt aktive Blatt (`currentSheet`) wird das erste Blatt.
- `text:textControl shrink="true"`: Text wird verkleinert (Schritte 8 %, min. 4 pt), bis er
  in seinen Rahmen passt – gemessen mit dem LabelLab-Renderer.
- Bandbreite: nächste Standardbreite (3/6/9/12/18/24/36 mm).
- Hinweise (`lbx.warn.<kind>`): `portrait`, `unknown`, `frame`, `clipart`, `font`,
  `vertical_text`, `barcode`, `image`, `crop`. Gleiche Hinweise werden nur einmal gemeldet.
- Geprüft mit den beiden Beispieldateien des Nutzers (lokal, nicht im Repo): Werkzeug-Labels
  (Bild, Logo, Text mit Verkleinern) sehen wie im Original aus; das Layout-Beispiel (Barcodes,
  QR, Tabelle, Datum/Zeit) ist vollständig, Aztec wird QR, Zierrahmen ein Rechteck.

## Grenzen
- Kein Export nach `.lbx`. Hochformat-Layouts werden als Querformat übernommen.
- Schriften, die nicht installiert sind, werden durch die Standardschrift ersetzt (Hinweis).
