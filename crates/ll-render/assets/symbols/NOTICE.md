# Symbol-Quelle

Die SVG-Dateien in diesem Verzeichnis stammen aus **Material Symbols** (Google), bezogen über
das npm-Paket [`@material-symbols/svg-400`](https://www.npmjs.com/package/@material-symbols/svg-400)
(Variante „outlined", Gewicht 400), Version 0.47.5.

Lizenz: **Apache License 2.0** — <https://www.apache.org/licenses/LICENSE-2.0>

Unverändert übernommen, nur umbenannt (siehe `ll_render::symbols` für die Zuordnung
Originalname → interner Name):

| Datei | Original (Material Symbols) |
|---|---|
| `network.svg` | `lan` |
| `wifi.svg` | `wifi` |
| `power.svg` | `bolt` |
| `warning.svg` | `warning` |
| `arrow_up.svg` | `arrow_upward` |
| `arrow_down.svg` | `arrow_downward` |
| `arrow_left.svg` | `arrow_back` |
| `arrow_right.svg` | `arrow_forward` |
| `fire.svg` | `local_fire_department` |
| `fire_extinguisher.svg` | `fire_extinguisher` |

Kein elektrisches Erdungssymbol enthalten — Material Symbols ist ein allgemeines UI-Icon-Set,
kein Satz elektrotechnischer Schaltzeichen. Eigene Symbole können per `labellab print --image
pfad.svg` unabhängig von dieser Bibliothek verwendet werden.
