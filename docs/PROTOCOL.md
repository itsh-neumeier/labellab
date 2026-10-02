# Druckerprotokoll (Brother P-touch Raster Command Reference)

> Jeder Eintrag trägt eine Quelle und einen Status. **Unverifizierte Werte nicht als gesichert
> behandeln** (siehe `AGENTS.md`). Status: `verifiziert` (Doku + Hardware-Test), `dokumentiert`
> (nur aus Treiber/Referenz, noch kein Hardware-Test), `unverifiziert` (Annahme/TODO).

## Identifikation

| Fakt | Wert | Status | Quelle |
|---|---|---|---|
| USB VID/PID PT-P710BT | `04F9:20AF` | dokumentiert | Brother-Treiber `Setup.ini` |
| USB VID/PID PT-P715eBT | `04F9:20C5` | dokumentiert | Brother-Treiber `Setup.ini` |
| USB VID/PID PT-E720BT | `04F9:224A` | dokumentiert | Brother-Treiber `Setup.ini` |
| Status-Byte Series PT-P710BT | `0x30` | dokumentiert | Brother Sprachmonitor-INI |
| Status-Byte Model PT-P710BT | `0x76` | dokumentiert | Brother Sprachmonitor-INI |
| Status-Byte Model PT-P715eBT | `0x77` | dokumentiert | Brother Sprachmonitor-INI |
| Status-Byte Model PT-E720BT | `0x81` | dokumentiert | Brother Sprachmonitor-INI |
| Invalidate-Länge | 100 × `0x00` | dokumentiert | Brother Sprachmonitor-INI (`CmdNull1=100`) |

## Bluetooth

| Fakt | Wert | Status | Quelle |
|---|---|---|---|
| Profil | Classic BT 2.1+EDR, SPP | verifiziert | Hardware-Test 2026-10-01 |
| SPP-UUID | `00001101-0000-1000-8000-00805F9B34FB` | verifiziert | Hardware-Test 2026-10-01 (natives RFCOMM, PT-P710BT meldet Dienstnamen „SPP SERVER“) |
| PIN | ggf. `0000` (`BT_DEFAULT_PIN`, für programmatische Kopplung) | unverifiziert | Annahme; Hardware-Test offen (PROGRESS) |
| RFCOMM-Kanal SPP (Linux/BlueZ, kein SDP-Lookup in `bluer`) | `1` (`BT_SPP_RFCOMM_CHANNEL`) | unverifiziert | Annahme (übliche SPP-Belegung); Windows ermittelt den Kanal per SDP selbst |
| Statusabfrage über ausgehenden BT-COM-Port (seriell) | `00×100, 1B 40, 1B 69 53` → 32 Byte, Byte0 `0x80` | **widerlegt** | Hardware-Test 2026-10-01: funktionierte einmalig auf einer anderen Maschine (COM9), scheitert auf der Zielmaschine reproduzierbar mit `ERROR_SEM_TIMEOUT`/`ERROR_INVALID_FUNCTION` (sowohl .NET `SerialPort` als auch `tokio-serial`). Serieller BT-SPP-Fallback gilt als unzuverlässig, siehe ADR-007. |
| Statusabfrage über natives WinRT-RFCOMM (kein virtueller COM-Port) | `00×100, 1B 40, 1B 69 53` → 32 Byte, Byte0 `0x80` | **verifiziert** | Hardware-Test 2026-10-01, PT-P710BT über `labellab status --bt`: 9 mm Band korrekt erkannt, `error1=0, error2=0, media_type=1, tape_color=1, text_color=8` |
| Eingehende BT-COM-Ports | unbrauchbar für Statusabfrage | verifiziert | Hardware-Test 2026-10-01 |
| Gleichzeitige BT-Verbindungen | nur eine (z. B. Handy blockiert PC) | verifiziert | Hardware-Test 2026-10-01 |
| Gerätename hinter dem SPP-Dienst | `RfcommDeviceService.Device().Name()` liefert den Kopplungsnamen (z. B. `PT-P710BT5265`) statt „SPP SERVER“ | unverifiziert | Annahme nach WinRT-Doku; Fallback auf Dienstnamen implementiert |

## USB

| Fakt | Wert | Status | Quelle |
|---|---|---|---|
| Interface-Klasse | Drucker (`0x07`), Bulk-OUT für Befehle/Raster, Bulk-IN für den 32-Byte-Status | unverifiziert | Annahme nach USB-Druckerklasse; `ll_transport::usb` sucht Endpunkte dynamisch statt fester Adressen |
| Windows-Treiberbindung | `nusb` braucht WinUSB am Interface (nicht `usbprint.sys`/Brother-Treiber) | unverifiziert | `nusb`-Doku; Hardware-Test offen |
| Linux-Zugriff ohne root | udev-Regel, z. B. `SUBSYSTEM=="usb", ATTRS{idVendor}=="04f9", ATTRS{idProduct}=="20af", MODE="0660", TAG+="uaccess"` in `/etc/udev/rules.d/60-labellab.rules` | unverifiziert | Annahme, Hardware-Test offen |

## Befehle

| Befehl | Bytes | Status | Quelle |
|---|---|---|---|
| Invalidate | `00` × 100 | verifiziert | Hardware-Test 2026-10-01 (`labellab print`) |
| Initialize | `1B 40` | verifiziert | Hardware-Test 2026-10-01 |
| Status-Request | `1B 69 53` | verifiziert | Hardware-Test 2026-10-01 |
| Raster-Modus | `1B 69 61 01` | verifiziert | Hardware-Test 2026-10-01: `labellab print "TEST" --bt` → lesbarer Druck auf 9-mm-Band |
| Druckinformation | `1B 69 7A n1..n10` | verifiziert (n1-Validitätsflags siehe TODO) | Hardware-Test 2026-10-01, Werte wie in `crates/ll-protocol/src/command.rs` |
| Various Mode (Auto-Cut Bit 6) | `1B 69 4D n` | verifiziert (nur mit `n=0`, ohne Auto-Cut, getestet) | Hardware-Test 2026-10-01 |
| Advanced Mode | `1B 69 4B n` | dokumentiert (nicht gesendet) | Raster Command Reference — `print_text()` sendet diesen Befehl aktuell nicht |
| Rand/Vorschub (Punkte, LE16) | `1B 69 64 n1 n2` | verifiziert (nur mit `n1=n2=0` getestet) | Hardware-Test 2026-10-01 |
| Kompression TIFF/PackBits | `4D 02` | verifiziert | Hardware-Test 2026-10-01 |
| Rasterzeile (Länge LE16) | `47 n1 n2 <daten>` | verifiziert | Hardware-Test 2026-10-01 |
| Leerzeile | `5A` | verifiziert | Hardware-Test 2026-10-01 |
| Drucken mit Vorschub (letzte Seite) | `1A` | verifiziert | Hardware-Test 2026-10-01 |
| Seite ohne Vorschub | `0C` | dokumentiert (nicht gesendet) | Raster Command Reference — nur für Kettendruck (M7) relevant |
| Vorschnitt per Leerseite (1 Zeile + Auto-Cut) vor dem Label | ergab **3 Schnitte** statt einem | **widerlegt / entfernt** | Hardware-Test 2026-10-01 (Nutzer). Deutung: mit Auto-Cut schneidet der Drucker den Vorlauf am Auftragsanfang selbst (unverifiziert, Test offen) |
| Mehrseitiger Auftrag | Init einmal; je Seite Steuercodes (Raster-Modus, Various Mode, Rand, Druckinfo mit `n9` = 0 erste / 1 weitere Seite, Kompression), Raster, dann `0C` (weitere Seite folgt) bzw. `1A` (letzte) | dokumentiert | Raster Command Reference PT-E550W/P750W/P710BT v1.02, Kap. 2.1 |
| „Schnitt nach je n Labels“ `ESC i A n` | vom PT-P710BT **nicht** unterstützt | dokumentiert | Raster Command Reference v1.02 („The PT-P710BT does not support this command“) — daher Kettendruck: Auto-Cut nur in den Steuercodes der letzten Seite (unverifiziert) |
| Half-Cut (`ESC i K` Bit 2) | beim PT-P710BT nicht verwendet | dokumentiert | Raster Command Reference v1.02 |
| Advanced Mode Bit 3 „No chain printing“ | 1 = nach dem letzten Label vorschieben und schneiden, 0 = nicht | dokumentiert (nicht gesendet) | Raster Command Reference v1.02 |

## Band- und Schriftfarbe (Statusbyte 24/25)

Quelle: Raster Command Reference PT-E550W/P750W/P710BT v1.02, Tabellen (8)/(9); umgesetzt in
`crates/ll-protocol/src/media.rs`. Status: dokumentiert; **hardware-bestätigt nur `01`/`08`**
(weißes Band, schwarze Schrift, PT-P710BT 2026-10-01).

| Bandfarbe (Byte 24) | Code | | Schriftfarbe (Byte 25) | Code |
|---|---|---|---|---|
| Weiß | `01` | | Weiß | `01` |
| Sonstige | `02` | | Sonstige | `02` |
| Transparent | `03` | | Rot | `04` |
| Rot | `04` | | Blau | `05` |
| Blau | `05` | | Schwarz | `08` |
| Gelb | `06` | | Gold | `0A` |
| Grün | `07` | | Blau (F) | `62` |
| Schwarz | `08` | | Reinigung / Schablone / inkompatibel | `F0` / `F1` / `FF` |
| Transparent (weiße Schrift) | `09` | | | |
| Matt Weiß / Matt Transparent / Matt Silber | `20` / `21` / `22` | | | |
| Satin Gold / Satin Silber | `23` / `24` | | | |
| Blau (D) / Rot (D) | `30` / `31` | | | |
| Neon-Orange / Neon-Gelb | `40` / `41` | | | |
| Beerenrosa / Hellgrau / Limettengrün (S) | `50` / `51` / `52` | | | |
| Gelb / Pink / Blau (F) | `60` / `61` / `62` | | | |
| Weiß (Schrumpfschlauch) | `70` | | | |
| Weiß / Gelb (Flex. ID) | `90` / `91` | | | |
| Reinigung / Schablone / inkompatibel | `F0` / `F1` / `FF` | | | |

## Statusblock (32 Byte)

| Byte | Bedeutung | Status | Quelle |
|---|---|---|---|
| 0 | `0x80` (Druckstatus-Antwort) | verifiziert | Hardware-Test 2026-10-01 |
| 8 | Fehler 1 | verifiziert (Feld existiert, Bit-Bedeutung offen) | Hardware-Test 2026-10-01: `0x00` bei fehlerfreiem Status |
| 9 | Fehler 2 | verifiziert (Feld existiert, Bit-Bedeutung offen) | Hardware-Test 2026-10-01: `0x00` bei fehlerfreiem Status |
| 10 | Bandbreite (mm) | verifiziert | Hardware-Test 2026-10-01 (9 mm korrekt erkannt, zweimal bestätigt) |
| 11 | Medientyp | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x01` bei eingelegtem 9-mm-Band |
| 18 | Statustyp (0x00 Antwort, 0x01 Druck fertig, 0x02 Fehler, 0x06 Phasenwechsel) | verifiziert (0x00) | Hardware-Test 2026-10-01: `0x00`/„Antwort“ bei Statusabfrage ohne Druckauftrag |
| 19 | Phase | verifiziert (Feld existiert, Sub-Codes offen) | Hardware-Test 2026-10-01: `0x00` im Ruhezustand |
| 24 | Bandfarbe | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x01` |
| 25 | Schriftfarbe | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x08` |

| 3 / 4 | Serien-/Modellcode (`0x30`/`0x76` = PT-P710BT) | dokumentiert | Treiber-INI, `model.rs` |
| 8/9 Bits | Fehler: 8: `0x01` kein Band, `0x04` Messer, `0x08` Akku schwach, `0x40` Netzteil; 9: `0x01` falsches Band, `0x10` Abdeckung offen, `0x20` Überhitzung | dokumentiert, TODO(verify) | Raster Command Reference; `status::ERROR_BITS` |
| 11 Codes | `0x00` kein Band, `0x01` laminiert, `0x03` nicht laminiert, `0x11`/`0x17` Schrumpfschlauch 2:1/3:1, `0xFF` inkompatibel | dokumentiert (`0x01` am Gerät gesehen) | Raster Command Reference; `media::MEDIA_TYPES` |
| 22 | Meldung: `0x01` Abdeckung geöffnet, `0x02` geschlossen | dokumentiert, TODO(verify) | Raster Command Reference |

- Rohdaten vom Nutzer (2026-10-02, PT-P710BT, 24-mm-Band, Stromzustand nicht angegeben):
  `80 20 42 30 76 30 00 00 | 00 00 18 14 01 00 00 00 | 00×8 | 90 08 00 00 00 00 00 00`.
  Damit **verifiziert**: Byte 1 `0x20` (Größe), 2 `0x42` („B“), 3/4 `0x30`/`0x76` =
  PT-P710BT, 10 = 24 mm, 25 `0x08` = schwarz. Beobachtet: Medientyp (11) `0x14` — nicht in
  der dokumentierten Tabelle (unbekannt, evtl. Flex-Band; TODO(verify)), Byte 12 `0x01`,
  Bandfarbe (24) `0x90` = laut Tabelle „Weiß (Flex)“. Fehlerbytes 0.
- TODO(verify): Status-Byte für Akkustand vorhanden? Rohdaten im Dialog „Druckerinfo“ am
  Ladekabel und im Akkubetrieb vergleichen. (siehe `docs/PROGRESS.md` → Hardware-Tests offen)
- TODO(verify): Code-Bedeutung Medientyp `0x01`, Bandfarbe `0x01`, Schriftfarbe `0x08` (vermutlich
  laminiert/schwarz auf weiß, gegen Brothers Farbcode-Tabelle prüfen)

## Kopf & Bandgeometrie

| Fakt | Wert | Status | Quelle |
|---|---|---|---|
| Auflösung | 180 dpi (optional 180×360 hohe Auflösung) | dokumentiert | Raster Command Reference |
| Druckkopf-Pins | 128 (16 Byte/Rasterzeile) | dokumentiert | Raster Command Reference |

Bedruckbare Pins / linker Pin-Offset je Bandbreite (Startwerte aus `MASTER_PROMPT.md`, noch gegen
Referenz und Hardware zu prüfen — siehe `crates/ll-protocol/src/model.rs`):

| Bandbreite | Pins | Offset | Status |
|---|---|---|---|
| 3,5 mm | 24 | 52 | unverifiziert |
| 6 mm | 32 | 48 | unverifiziert |
| 9 mm | 50 | 39 | teilweise verifiziert (`labellab print` druckte lesbaren Text auf echtem 9-mm-Band, 2026-10-01; Pin-Zahlen nicht einzeln nachgemessen) |
| 12 mm | 70 | 29 | unverifiziert |
| 18 mm | 112 | 8 | unverifiziert |
| 24 mm | 128 | 0 | unverifiziert |

- Rand/Vorschub `1B 69 64`: wirkt nach einer Messung **vor und nach** den Rasterdaten
  (Label mit fester Länge 100 mm und Rand 28 Punkte ≈ 3,95 mm kam ~107 mm lang heraus,
  Nutzer 2026-10-02). Status: **unverifiziert** (eine Messung). Darum zieht `print.rs`
  (`trim_for_margin`) den Rand von den leeren Enden des Labels ab. TODO(verify): Label
  100 mm fest, Rand links/rechts 4 mm → gedruckte Länge 100 mm? Standard-Rand seitdem
  0 Punkte (Abstand zum Schnitt nur über die Label-Ränder).
- TODO(verify): Half-Cut am PT-P710BT unterstützt? (vermutlich nein)
- TODO(verify): Setzt eine Statusabfrage (`00×100, 1B 40, 1B 69 53`) den Timer der
  automatischen Abschaltung zurück? Annahme für „Wach halten“ in der App (alle 2 min,
  `KEEPALIVE_MS` in `app/src/main.ts`), unverifiziert; Abschaltzeit des Geräts ebenfalls
  nicht nachgemessen.
- TODO(verify): maximale Bluetooth-Durchsatzrate / sinnvolle Blockgröße beim Senden der Rasterdaten

## Ablauf eines Druckjobs

1. Verbinden → Invalidate → Initialize → Status lesen → Band prüfen (Breite passt zum Layout?)
2. Raster-Modus, Various/Advanced Mode, Rand, Druckinfo, Kompression
3. Rasterzeilen senden (gestreamt, PackBits, Leerzeilen als `5A`)
4. `1A` → auf Status „Druck fertig“ bzw. „Fehler“ warten (Timeout konfigurierbar) → Ergebnis an UI
