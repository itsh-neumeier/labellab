# Entwurf: Rohrleitungskennzeichnung nach DIN 2403 (wartet auf Freigabe des Nutzers)

Stand: 2026-10-03. Nutzerwunsch: Vorlagen-Assistent für Rohrleitungskennzeichnung, Pfeilrichtung
immer wählbar, Gefahrensymbole wie in der DIN; erst Entwurf, nach Freigabe umsetzen. Später:
eigenes Icon-Set mit den GHS-Piktogrammen.

## Grundidee (einfarbiger Druck)
Der Drucker druckt nur eine Farbe (die der Kassette) auf die Bandfarbe. DIN-Optik entsteht so:
- **Bandfarbe = Stofffarbe** (Kassette nach Gruppe wählen, z. B. weiß auf grün für Wasser).
- LabelLab druckt **alles außerhalb der Pfeilform** in der Druckfarbe (= weißer/schwarzer Rand),
  dazu Trennstriche zwischen Spitze und Körper, Text und Symbole. Die Pfeilform selbst bleibt
  frei und zeigt die Bandfarbe.
- **Zusatzfarbe** (Gruppen 4/5/8/9): Schwarz ist druckbar (Spitzen gefüllt, Gruppen 5 und 9).
  Rot (Gruppen 4 und 8) ist mit einer Kassette nicht druckbar → Spitzen frei lassen oder
  schraffieren, Hinweis im Assistenten.

## Stoffgruppen (DIN 2403, Tabelle aus der Vorlage des Nutzers)
| Gr. | Stoff | Farbe | Schrift/Rand | Zusatzfarbe |
|---|---|---|---|---|
| 0 | Sauerstoff | blau RAL 5005 | weiß | – |
| 1 | Wasser | grün RAL 6032 | weiß | – |
| 2 | Dampf | rot RAL 3001 | weiß | – |
| 3 | Luft | grau RAL 7004 | schwarz | – |
| 4 | brennbare Gase | gelb RAL 1003 | schwarz | rot |
| 5 | nicht brennbare Gase | gelb RAL 1003 | schwarz | schwarz |
| 6 | Säuren | orange RAL 2010 | schwarz | – |
| 7 | Laugen | violett RAL 4008 | weiß | – |
| 8 | brennbare Flüssigkeiten | braun RAL 8002 | weiß | rot |
| 9 | nicht brennbare Flüssigkeiten | braun RAL 8002 | weiß | schwarz |

Bei Farben ohne passende Kassette (grau, violett, braun) schlägt der Assistent die nächstbeste
vor und weist darauf hin.

## Assistent (Kachel „Rohrleitung (DIN 2403)“, neue Rubrik „Anlagen & Rohrleitungen“)
- Stoffgruppe 0–9 (setzt Farben, Zusatzfarbe, empfohlene Kassette, Vorschau in Bandfarben)
- Medium (Text, Vorschläge je Gruppe: Trinkwasser, Heizung Vorlauf/Rücklauf, Kaltwasser …)
- Zusatzzeile optional (z. B. „80 °C · PN 10“)
- **Pfeilrichtung (Pflicht):** rechts, links, beidseitig
- Gefahrensymbole: bis zu 3, aus dem GHS-Icon-Set (GHS01–GHS09)
- Länge (Vorschlag aus Bandbreite, frei änderbar), Rand: ausgefüllt oder nur Kontur
- Ziel wie bei anderen Vorlagen (einfügen, neues Blatt …); nachträglich bearbeitbar

## Technik
- Neues Element `pipe_marker` (wie Verteilerbeschriftung als Spezial-Element): Text, Zusatzzeile,
  Richtung, Spitzenfüllung, Symbolliste, Rand-Stil; Rendern über den normalen Renderpfad
  (Polygon für die Pfeilform, Text über den Textpfad, Symbole über die Icon-Sets).
- GHS-Icon-Set: GHS01–GHS09 als eigene SVGs (Raute + Symbol, einfarbig druckbar), wie die
  vorhandenen Sets als `.llabel-iconset`.
- Entwurfsbild: vom Agenten erzeugte Mock-ups (nicht im Repo).
