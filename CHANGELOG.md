# Changelog

Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung nach [SemVer](https://semver.org/lang/de/).

## [Unreleased]
### Hinzugefügt
- „A4-Druck …“: Blätter der Arbeitsmappe auswählen (mit Anzahl) und auf A4 mit einem
  normalen Drucker drucken – Kopfzeile mit LabelLab-Logo, Graustufen, Farben wie Band /
  eigene Hintergrund- und Schriftfarbe / Schwarz auf Weiß, Schnittlinien, Abstand. Testseite
  mit 100-mm-Linealen; gemessene Werte ergeben einen gespeicherten Korrekturwert (waagerecht
  und senkrecht)
- Einstellungen und eigene Daten liegen bei der Programmdatei: `LabelLab.settings`
  (verschlüsselt, Änderungen von außen werden erkannt und verworfen) und der Ordner
  `llappdata` (Rahmen, Icon-Sets, Druckverlauf, eingefügte Bilder). Vorhandene Daten aus
  `%APPDATA%\LabelLab` werden beim ersten Start übernommen. Ist der Programmordner nicht
  beschreibbar (z. B. installiert unter „Programme“), bleibt alles im Benutzerordner
- Zusätzlich gemerkt: zuletzt gewählter Drucker, Modell, Band, Mehrband, Kopien, Nachlauf,
  Schnittmarken, Spiegeln, Zoom und Vorschau-Qualität
- Projektstart: README, MIT-Lizenz, Master-Prompt, Multi-Agent-Struktur, Bluetooth-Diagnose-Skript
- M1: Cargo-Workspace mit `ll-protocol`, `ll-transport`, `ll-render`, `ll-core`, `ll-cli`; CI
  (fmt/clippy/test auf Windows + Linux); `docs/PROTOCOL.md`, `docs/ARCHITECTURE.md`
- M2: Status-Parser, Befehlsaufbau und PackBits-Encoder in `ll-protocol`; Serial-Transport in
  `ll-transport`; CLI `labellab devices` und `labellab status [--device COM<n>] [--json]`
- M4 (Windows-Teil, vorgezogen): natives Bluetooth-RFCOMM über WinRT als Ersatz für den
  unzuverlässigen seriellen BT-SPP-Fallback; `labellab devices` listet gekoppelte
  Bluetooth-Geräte, `labellab status --device <ID> --bt`
  — hardware-verifiziert gegen echten PT-P710BT
- M3: `labellab print "Text" --device <...> [--bt] [--cut] [--copies N]` — erster echter
  Druckjob (Status lesen, Band automatisch erkennen, Platzhalter-Bitmapfont rendern, PackBits,
  an den Drucker senden) — hardware-verifiziert: druckt lesbaren Text auf echtes Band
- M5 (Teil): echte Systemschriften (`fontdue`) ersetzen den M3-Platzhalter-Font;
  `labellab render "Text" -o vorschau.png --width <mm>` für Vorschau ohne Drucker (gleicher
  Renderpfad wie der Druck) — hardware-verifiziert
- M5 (Teil): QR-Codes (`qrcode`-Crate) — `labellab print --qr "..."` und
  `labellab render --qr "..." -o vorschau.png` — hardware-verifiziert (gedruckt + gescannt)
- M5 (Teil): Code128-Barcode (`barcoders`-Crate) — `labellab print --barcode "..."` und
  `labellab render --barcode "..." -o vorschau.png` — gedruckt (sauberes Balkenmuster),
  Scan-Lesbarkeit noch nicht verifiziert
- M5 (Teil): Bildimport (PNG/JPEG/BMP) mit Floyd-Steinberg-Dithering — `labellab print --image
  <datei> [--invert]` und `labellab render --image <datei> -o vorschau.png`
- M4 (Teil): USB-Transport (`nusb`) für Windows und Linux — `labellab devices` listet
  angeschlossene USB-Drucker, `labellab status --usb` und `labellab print ... --usb
  [--device <Modell|VVVV:PPPP|Seriennummer>]`; noch nicht hardware-getestet
- M6 (Teil): Vorlagenformat `.llabel` (JSON) mit mehreren Elementen pro Label (Text, QR,
  Barcode, Bild), Abstand, Rand, Mindestlänge und Rahmen — `labellab print/render --template
  <datei.llabel>`
- M6 (Teil): erste Desktop-Oberfläche (Tauri 2) mit Editor, Live-Vorschau, Geräteauswahl,
  Bandstatus, Drucken, Öffnen/Speichern, Rückgängig/Wiederholen, Deutsch/Englisch
- Editor: Elemente frei als Boxen platzieren und skalieren (Maus, Pfeiltasten, X/Y/Breite/Höhe),
  Einrasten an Bandkanten und anderen Boxen zum bündigen Aneinanderlegen, Duplizieren
- Text: Schriftgröße in pt oder automatisch, mehrzeilig, Ausrichtung links/Mitte/rechts
- Schriftarten aus den installierten Systemschriften, fett und kursiv (fehlende Schnitte werden
  nachgebildet)
- Serien aus CSV: Platzhalter `{{Spalte}}` und `{{#}}`, Vorschau je Datensatz, Druck aller oder
  eines Bereichs (GUI und `labellab print --csv … --rows 1-10`)
- Mehrband-Labels: Label über 2×, 3× oder 4× Band übereinander gestalten, gedruckt als ein
  Streifen pro Band
- Fortlaufender Druck („Fortlaufend“, `--chain`): Serien, Kopien und Streifen in einem Auftrag
  ohne Schnitt dazwischen, optional ein Schnitt am Ende
- Vorschau in allen gängigen Band-/Schriftfarben (schwarz auf weiß/gelb/transparent, weiß auf
  schwarz, rot/blau/gold …), nach „Status lesen“ automatisch passend zum eingelegten Band
- Glatte, hochauflösende Vorschau (umschaltbar auf das exakte Druckraster)
- Gerätesuche zeigt den Bluetooth-Gerätenamen statt „SPP SERVER“, erkennt den Drucker, wählt ihn
  aus und liest den Bandstatus automatisch
- Druckknopf zeigt „Wird gedruckt … n/m“ und ist während der Übertragung gesperrt
- Vorlagen-Assistent und `labellab generate`: Kabelfahne (Wickelbereich π × Durchmesser),
  Kabelwickel (Text wiederholt, optional gedreht), Patchpanel/Port-Labels (festes Raster,
  Nummerierung, Trennstriche)
- Nummernfolgen ohne CSV: `{{n}}`, `{{n:03}}`, `{{A}}`/`{{a}}` mit Start/Schritt/Anzahl
  (`--count/--start/--step`, Bereich „Nummerierung“)
- Elemente drehen (90°-Schritte), neues Element „Linie/Fläche“, Symbol-Element in der Oberfläche
- Symbole aus der mitgelieferten Bibliothek auch in Vorlagen und Serien (`{"type": "symbol"}`)
- Vorlagenformat `.llabel` Version 2 (Boxen, Textgröße/-ausrichtung); Version 1 wird weiter gelesen
- Windows: portable `LabelLab.exe` (ohne Installation startbar) und `labellab.exe` werden bei
  jedem Push automatisch gebaut (GitHub Actions, Artefakt `LabelLab-windows-x64-portable`)
- M5 (Teil): Rahmen um das ganze Label — `--frame` auf `print` und `render`, gilt für
  Text/QR/Barcode/Bild
- M5 (Teil): EAN-13, EAN-8, UPC-A, Code39, ITF als weitere Barcode-Symbologien — `--barcode-type
  <code128|ean13|ean8|upca|code39|itf>` auf `print`/`render`. Nur PNG-Vorschau geprüft, noch
  nicht auf Band gedruckt.
- M5 (Teil): SVG-Import über denselben `--image`-Pfad (`resvg`/`usvg`/`tiny-skia`, an `.svg`-
  Endung erkannt). Nur PNG-Vorschau geprüft, noch nicht auf Band gedruckt.
- M5: Symbolbibliothek — 10 eingebettete Material-Symbols-Icons (Apache-2.0: `network`, `wifi`,
  `power`, `warning`, `arrow-up/-down/-left/-right`, `fire`, `fire-extinguisher`) über
  `--symbol <name>` auf `print`/`render`, `labellab symbols` listet verfügbare Namen. Eigene
  SVGs bleiben unabhängig davon über `--image` nutzbar. **M5 damit code-seitig vollständig**;
  nur PNG-Vorschau geprüft, noch nicht auf Band gedruckt.
- M4: Bluetooth unter Linux (BlueZ) und Kopplung des Druckers direkt aus LabelLab
  (`labellab pair`, in der Oberfläche „Koppeln …“) unter Windows und Linux — noch nicht am
  Gerät getestet
- Oberfläche: verständliche deutsche Fehlermeldungen (z. B. „Der Drucker antwortet nicht …“)
  und Warnung, wenn Text nicht in seine Box passt und abgeschnitten wird
- Oberfläche: Liste „Zuletzt verwendet“ für Label-Dateien und Übersicht „Serie ansehen …“ mit
  allen Labels einer CSV-/Nummern-Serie
- Rahmen in mehreren Stilen (durchgezogen, gestrichelt, gepunktet, doppelt, gestreift wie
  Warnband) mit einstellbarer Linienstärke, Abstand und frei wählbaren Seiten (z. B. nur oben
  oder oben und unten) — in der Oberfläche und per `labellab … --border …`
- Schriftauswahl zeigt jede Schrift in ihrem eigenen Schriftbild, mit Suchfeld
- Icon-Sets: Symbole nach Sets und Kategorien geordnet, neue Symbolauswahl mit Suche und
  Vorschau; eigene Sets als `.llabel-iconset`-Datei importieren (auch aus einem Ordner mit
  SVGs erstellbar: `labellab iconset create`)
- Mitgelieferte Icon-Sets „ISO 7010 Sicherheitszeichen“ (335 Warn-, Verbots-, Gebots-,
  Brandschutz- und Rettungszeichen) und „IEC 60417 Gerätesymbole“ (754 Symbole, z. B. Erde,
  Sicherung, Ein/Aus, Schutzklasse II), Grafiken von Wikimedia Commons (gemeinfrei/CC0)
- Editor: Formen (Linie, Rechteck, abgerundetes Rechteck, Oval; Kontur oder gefüllt),
  „Länge fest“ (Inhalt wird auf die Mindestlänge abgeschnitten), Platzhalter `{{datum}}` und
  `{{zeit}}`, Helligkeit/Kontrast für Bilder, Elemente sperren, Ausrichten-Knöpfe, Lineal in mm,
  Umschalt beim Skalieren hält das Seitenverhältnis
- Vorlagen-Galerie („Vorlagen …“) mit Live-Vorschau und neuen Vorlagen: Einzelfähnchen,
  Klemmblock/LSA-Leiste (zweireihig), Sicherungskasten/Verteiler (senkrechte Beschriftung,
  Hauptschalter-Feld); auch per `labellab generate single-flag|terminal-block|fuse-box`
- Druckverlauf „Verlauf …“: die letzten 50 gedruckten Labels mit Vorschau, Datum und Band,
  per Klick wieder öffnen
- Mehrere Arbeitsblätter in einer `.llabel`-Datei (Reiter über der Vorschau; CLI
  `--sheet`), Warnung vor ungespeicherten Änderungen beim Schließen, Neu und Öffnen
- Bild-Editor „Bild bearbeiten …“: zuschneiden, drehen, spiegeln, Hintergrund entfernen
  (automatisch oder per Klick auf eine Farbe, mit Toleranz), Raster oder Schwelle für den
  Druck, mit Druckvorschau; das Originalbild bleibt unverändert
- Strg+V fügt Bilder und Texte aus der Zwischenablage als neue Elemente ein; Strg+C, Strg+X
  und Strg+V kopieren, schneiden und fügen das gewählte Element ein
- „Beispiel-CSV …“: erzeugt eine CSV mit den Platzhalter-Spalten des Labels, speichert und
  lädt sie
- Schnittoptionen: jedes Etikett, Schnitt am Ende, alle N Etiketten, Kettendruck ohne
  Schnitt, kein Schnitt (Spezialband); dazu Schnittmarken und Spiegeldruck
- Deko-Rahmen aus Anfangs-, Mittel- (wiederholt) und Endstück: 8 mitgelieferte Rahmen,
  eigener Rahmen-Editor (SVG), Import von Rahmen-Sets (`.llabel-frames`)
- Hochformat: Label mit senkrechtem Band bearbeiten („Ausrichtung“ unter Label)
- Elemente können eigene Namen bekommen (Doppelklick in der Liste)
- Editor-Aufbau wie Ebenen: links die Elementliste mit Ein-/Ausblenden, Sperren,
  Duplizieren und Löschen, rechts die Eigenschaften des gewählten Elements; ausgeblendete
  Elemente werden nicht gedruckt
- Auto-Speichern: nach dem ersten Speichern werden Änderungen automatisch in die Datei
  geschrieben (Schalter in der Werkzeugleiste); „Speichern“ speichert direkt,
  „Speichern unter …“ fragt nach dem Ort; Rückgängig/Wiederholen als Symbole
- Linke Leiste übersichtlicher: Bereiche (Rahmen, Nummerierung, Daten …) lassen sich per
  Klick auf die Überschrift auf- und zuklappen
- „Druckerinfo …“: Modell, Band, Bandtyp, Farben, Fehler im Klartext und die Rohdaten des
  Druckerstatus (kopierbar)
- Startbildschirm und „Über …“ mit Entwickler, Copyright und Lizenz
- Inhalt in der Box ausrichten: links/mitte/rechts und oben/mitte/unten für Text, Codes,
  Bilder und Symbole (Zeile „Inhalt:“)
- Rahmen: Abstand für jede Seite einzeln; der Rahmen liegt innerhalb von Rand links/rechts
- Senkrechtes Lineal links der Vorschau (mm über die ganze Bandbreite)
- Element-Boxen lassen sich an allen Seiten und Ecken in der Größe ändern (vorher nur
  rechts, unten und unten rechts)
- Rand links und rechts getrennt einstellbar (statt nur „Rand am Ende“); die Vorschau
  zeigt die Ränder als Zonen an beiden Enden, Ausrichten und Einrasten halten sie ein,
  Elemente im Rand werden gemeldet
- Knopf „Wach halten“: fragt alle 2 Minuten den Druckerstatus ab, damit sich der Drucker
  nicht automatisch ausschaltet (beim Start aus; Wirkung am Gerät noch ungeprüft)
- Einzelne Textteile fett oder kursiv: Text markieren und F/K (Strg+B/Strg+I) drücken;
  gespeichert als `**fett**` und `__kursiv__` im Text
- Zeilenabstand für Text einstellbar (Feld „Zeilenabstand“, 0,5–3)
- Knopf „Code“ (Code-Assistent) ersetzt die Knöpfe QR-Code und Barcode: Code-Typ wählen (QR, Code 128,
  EAN-13, EAN-8, UPC-A, Code 39, ITF), für QR-Codes Inhalte wie WLAN-Zugang, Kontakt (vCard),
  E-Mail, Telefon oder Link ausfüllen, Live-Vorschau mit Prüfung; bestehende Codes über
  „Assistent …“ in den Eigenschaften bearbeiten (auch QR ⇄ Barcode umstellen)
- Verteilerbeschriftung (vorher „Sicherungskasten“): Feldtexte mehrzeilig, Teile fett/kursiv
  (markieren + F/K bzw. Strg+B/Strg+I), Zeilenabstand, Ausrichtung waagerecht und senkrecht
  (Zeile „Inhalt:“)
- Neue Spezial-Elemente in „Vorlagen / Spezial-Elemente“: **Reihenklemmen** (20 × 5,2 mm,
  Nummern senkrecht) und **LSA-Leiste** (10 Felder × 10 mm); alles nachträglich änderbar
- Rahmen-Editor mit Zeichenfläche („kleines Paint“): Anfang, Mitte und Ende pixelgenau
  zeichnen (Stift, Radierer, Linie, Rechteck, Ellipse, Füllen, Strichstärke, Rückgängig,
  Spiegeln, „Ende = Anfang gespiegelt“, Breite, Zuschneiden), Bilder aus Datei oder
  Zwischenablage einfügen (mit Bild-Editor zuschneiden/freistellen, dann platzieren mit
  Größe, Schwelle, Raster); SVG-Code bleibt als Alternative
- „Vorlagen / Spezial-Elemente“: der Sicherungskasten wird dort angelegt (nicht mehr unter
  Hinzufügen); neues Ziel „Ins aktuelle Blatt einfügen“ (Standard beim Sicherungskasten)
- Knopf „✂ Schnitt“ neben „Drucken“: Band vorschieben und abschneiden, ohne zu drucken
  (z. B. nach „Kein Schnitt“)
- Neues Element „Sicherungskasten“ (wie im Hersteller-Editor): ein Element mit Feldern und
  Trennzeichen (Rahmen, Linie, Fett, Gestrichelt, Markierungen, keine), alles nachträglich in
  den Eigenschaften änderbar: Anzahl Felder, Raster, Breite je Feld (0,5×–8×), Text je Feld,
  Felder verbinden, Text senkrecht (auch je Feld), Umkehren, Schrift/Größe/Fett/Kursiv. Die
  Vorlage „Sicherungskasten / Verteiler“ erzeugt jetzt dieses Element
- Vorlage Sicherungskasten/Verteiler: Teileinheiten zu einem Feld verbinden (⇔) und wieder
  trennen (✂), eigener Text je Feld; das Label merkt sich die Vorlage und lässt sich über
  „Vorlage bearbeiten …“ jederzeit anpassen
- Vorlagen: Ziel wählbar – aktuelles Blatt ersetzen, als neues Blatt hinzufügen oder neue
  Datei
### Geändert
- Vorschau zeigt die volle Bandbreite; der Rand, den der Druckkopf nicht erreicht, ist grau
- Deutlich schnellere Vorschau bei vielen Elementen und großen Bildern; die Oberfläche
  bleibt während des Renderns bedienbar
- Texte der Oberfläche (Beschriftungen, Hinweise, Knöpfe) lassen sich nicht mehr markieren;
  Eingabefelder bleiben markierbar

### Behoben
- „Kein Schnitt“ / „Kettendruck ohne Schnitt“: der Drucker schnitt am Ende trotzdem. Jetzt
  wird der Vorschub-und-Schnitt nach dem letzten Label ausdrücklich abgeschaltet (am Gerät
  noch zu bestätigen)
- Symbole: „Vorschau nicht möglich … missing field `name`“ – der eigene Elementname kollidierte
  mit dem Symbolnamen; der Elementname wird jetzt als `title` gespeichert
- UPC-A: 11 Ziffern (oder 12 mit Prüfziffer) werden jetzt als UPC-A kodiert; vorher wurde die
  Eingabe als EAN-13 ohne führende 0 gelesen
- USB unter Windows: Der Drucker wird jetzt über den Windows-Druckertreiber angesprochen
  (vorher Fehler „incompatible driver is installed for this device“)
- Gedruckte Labels waren um etwa 8 mm länger als eingestellt (der Drucker-Nachlauf kam an
  beiden Enden dazu); er wird jetzt von den Label-Rändern abgezogen, Standard ist 0
- Bild-Editor: Die Abdunklung des Zuschnitt-Rahmens lag über dem ganzen Dialog
- Windows-Paket: Die Kommandozeile heißt jetzt `labellab-cli.exe`. Vorher überschrieb
  `labellab.exe` beim Packen die Oberfläche `LabelLab.exe` (Windows unterscheidet keine
  Groß-/Kleinschreibung), das Artefakt enthielt nur die CLI.
- `labellab` brach nach dem Zusammenführen mit der Symbolbibliothek beim Start ab
  (`--symbol` widersprach sich selbst)
- Vorschau blieb in der Windows-Oberfläche leer
- Cutter schnitt direkt am Ende des gedruckten Inhalts ohne Nachlauf (`margin(0)` war fest
  einprogrammiert). `--margin <dots>` auf `print` macht den Leervorschub vor dem Schnitt
  konfigurierbar, Default jetzt 28 statt 0 Druckpunkte. Noch nicht erneut hardware-getestet.
