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
| PIN | ggf. `0000` | unverifiziert | Annahme |
| Statusabfrage über ausgehenden BT-COM-Port (seriell) | `00×100, 1B 40, 1B 69 53` → 32 Byte, Byte0 `0x80` | **widerlegt** | Hardware-Test 2026-10-01: funktionierte einmalig auf einer anderen Maschine (COM9), scheitert auf der Zielmaschine reproduzierbar mit `ERROR_SEM_TIMEOUT`/`ERROR_INVALID_FUNCTION` (sowohl .NET `SerialPort` als auch `tokio-serial`). Serieller BT-SPP-Fallback gilt als unzuverlässig, siehe ADR-007. |
| Statusabfrage über natives WinRT-RFCOMM (kein virtueller COM-Port) | `00×100, 1B 40, 1B 69 53` → 32 Byte, Byte0 `0x80` | **verifiziert** | Hardware-Test 2026-10-01, PT-P710BT über `labellab status --bt`: 9 mm Band korrekt erkannt, `error1=0, error2=0, media_type=1, tape_color=1, text_color=8` |
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
| 8 | Fehler 1 | verifiziert (Feld existiert, Bit-Bedeutung offen) | Hardware-Test 2026-10-01: `0x00` bei fehlerfreiem Status |
| 9 | Fehler 2 | verifiziert (Feld existiert, Bit-Bedeutung offen) | Hardware-Test 2026-10-01: `0x00` bei fehlerfreiem Status |
| 10 | Bandbreite (mm) | verifiziert | Hardware-Test 2026-10-01 (9 mm korrekt erkannt, zweimal bestätigt) |
| 11 | Medientyp | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x01` bei eingelegtem 9-mm-Band |
| 18 | Statustyp (0x00 Antwort, 0x01 Druck fertig, 0x02 Fehler, 0x06 Phasenwechsel) | verifiziert (0x00) | Hardware-Test 2026-10-01: `0x00`/„Antwort“ bei Statusabfrage ohne Druckauftrag |
| 19 | Phase | verifiziert (Feld existiert, Sub-Codes offen) | Hardware-Test 2026-10-01: `0x00` im Ruhezustand |
| 24 | Bandfarbe | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x01` |
| 25 | Schriftfarbe | verifiziert (Feld existiert, Code-Bedeutung offen) | Hardware-Test 2026-10-01: `0x08` |

- TODO(verify): Status-Byte für Akkustand vorhanden? (siehe `docs/PROGRESS.md` → Hardware-Tests offen)
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
