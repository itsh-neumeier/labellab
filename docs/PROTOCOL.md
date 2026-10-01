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
| SPP-UUID | `00001101-0000-1000-8000-00805F9B34FB` | dokumentiert | Standard-SPP-UUID |
| PIN | ggf. `0000` | unverifiziert | Annahme |
| Statusabfrage über ausgehenden BT-COM-Port | `00×100, 1B 40, 1B 69 53` → 32 Byte, Byte0 `0x80` | verifiziert | Hardware-Test 2026-10-01, COM9 |
| Eingehende BT-COM-Ports | unbrauchbar für Statusabfrage | verifiziert | Hardware-Test 2026-10-01 |
| Gleichzeitige BT-Verbindungen | nur eine (z. B. Handy blockiert PC) | verifiziert | Hardware-Test 2026-10-01 |

## Befehle

| Befehl | Bytes | Status | Quelle |
|---|---|---|---|
| Invalidate | `00` × 100 | dokumentiert | Sprachmonitor-INI |
| Initialize | `1B 40` | verifiziert | Hardware-Test 2026-10-01 |
| Status-Request | `1B 69 53` | verifiziert | Hardware-Test 2026-10-01 |
| Raster-Modus | `1B 69 61 01` | dokumentiert | Raster Command Reference |
| Druckinformation | `1B 69 7A n1..n10` | dokumentiert | Raster Command Reference |
| Various Mode (Auto-Cut Bit 6) | `1B 69 4D n` | dokumentiert | Raster Command Reference |
| Advanced Mode | `1B 69 4B n` | dokumentiert | Raster Command Reference |
| Rand/Vorschub (Punkte, LE16) | `1B 69 64 n1 n2` | dokumentiert | Raster Command Reference |
| Kompression TIFF/PackBits | `4D 02` | dokumentiert | Raster Command Reference |
| Rasterzeile (Länge LE16) | `47 n1 n2 <daten>` | dokumentiert | Raster Command Reference |
| Leerzeile | `5A` | dokumentiert | Raster Command Reference |
| Drucken mit Vorschub (letzte Seite) | `1A` | dokumentiert | Raster Command Reference |
| Seite ohne Vorschub | `0C` | dokumentiert | Raster Command Reference |

## Statusblock (32 Byte)

| Byte | Bedeutung | Status | Quelle |
|---|---|---|---|
| 0 | `0x80` (Druckstatus-Antwort) | verifiziert | Hardware-Test 2026-10-01 |
| 8 | Fehler 1 | dokumentiert | Raster Command Reference |
| 9 | Fehler 2 | dokumentiert | Raster Command Reference |
| 10 | Bandbreite (mm) | verifiziert | Hardware-Test 2026-10-01 (9 mm korrekt erkannt) |
| 11 | Medientyp | dokumentiert | Raster Command Reference |
| 18 | Statustyp (0x00 Antwort, 0x01 Druck fertig, 0x02 Fehler, 0x06 Phasenwechsel) | dokumentiert | Raster Command Reference |
| 19 | Phase | dokumentiert | Raster Command Reference |
| 24 | Bandfarbe | dokumentiert | Raster Command Reference |
| 25 | Schriftfarbe | dokumentiert | Raster Command Reference |

- TODO(verify): Status-Byte für Akkustand vorhanden? (siehe `docs/PROGRESS.md` → Hardware-Tests offen)

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
| 9 mm | 50 | 39 | unverifiziert |
| 12 mm | 70 | 29 | unverifiziert |
| 18 mm | 112 | 8 | unverifiziert |
| 24 mm | 128 | 0 | unverifiziert |

- TODO(verify): Half-Cut am PT-P710BT unterstützt? (vermutlich nein)
- TODO(verify): maximale Bluetooth-Durchsatzrate / sinnvolle Blockgröße beim Senden der Rasterdaten

## Ablauf eines Druckjobs

1. Verbinden → Invalidate → Initialize → Status lesen → Band prüfen (Breite passt zum Layout?)
2. Raster-Modus, Various/Advanced Mode, Rand, Druckinfo, Kompression
3. Rasterzeilen senden (gestreamt, PackBits, Leerzeilen als `5A`)
4. `1A` → auf Status „Druck fertig“ bzw. „Fehler“ warten (Timeout konfigurierbar) → Ergebnis an UI
