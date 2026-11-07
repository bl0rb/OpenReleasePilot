# Claude-Auftrag: Studio-Design für OpenReleasePilot und OpenSitePilot

Stand: 7. November 2026 · Umsetzungsvorgabe für dieses Repository

## Auftrag und Umfang

Setze die Desktop-Oberfläche dieses Repositories nach der gemeinsamen Designrichtung **Studio – modern & markant** um. Ziel ist eine hochwertige Produktfamilie aus Weave, OpenVizPilot, KeyZapper, OpenReleasePilot und OpenSitePilot: gleiche gestalterische Regeln, aber aufgabengerechte Ansichten pro App. Arbeite ausschließlich im unten genannten Repository und erhalte dessen vollständigen aktuellen Funktionsumfang.

Dies ist ein konkreter UI-Implementierungsauftrag, keine Aufforderung zu einem weiteren Konzeptpapier. Lies zuerst die geltenden Repository-Anweisungen, prüfe Branch und Arbeitsbaum und erhalte fremde Änderungen. Verwende das vorhandene Tauri-2-/HTML-/CSS-/JavaScript-System. Kein Frameworkwechsel, keine neue Build-Kette, kein gemeinsames Monorepo und kein neues UI-Paket. Gemeinsame Regeln werden lokal umgesetzt; keine Imports aus Geschwister-Repositories. Backend- oder native Änderungen nur, soweit sie für die konkrete Darstellung/Fenstergröße erforderlich sind.

Die Aufgabe endet mit lokal geprüfter Umsetzung und einem knappen Ergebnisbericht. Commit, Push, PR-Veröffentlichung, Release, Installation oder produktive externe Aktionen sind durch diese Anleitung nicht zusätzlich beauftragt. Bereits separat erteilte Freigaben gelten weiter.

## Visuelle Referenz und ihre Grenzen

Zusätzliche lokale Referenz: [Klickbarer Studio-Entwurf](/Users/matze/.codex/visualizations/2026/11/07/01a1b59c-f00f-78b2-99cc-17498d931bc5/desktop-studio.html). Darin die unten genannte App auswählen und beide Erscheinungsbilder sowie die passenden Ansichten ansehen. Die Datei ist ein HTML-Fragment für die Chat-Vorschau, keine startfertige Anwendung. Die Spezifikation in dieser Markdown-Datei ist auch ohne Zugriff auf die Vorschau vollständig genug zur Umsetzung.

**Übernimm die Gestaltung, nicht die Demo-Implementierung.** Insbesondere:

- Die obere App-Auswahl, „Desktop Studio“, „Designkonzept“, Beschreibungen außerhalb des Fensters und der Vorschau-Schalter Hell/Dunkel gehören nicht in die App. Keine neue App-übergreifende Navigation bauen.
- Gezeichnete macOS-Fensterknöpfe sind Vorschau-Chrome. In der App native Titelleiste/Buttons und vorhandene Drag-Regionen respektieren; keine zweite Titelleiste oder nachgebauten Traffic Lights hinzufügen.
- Beispielnamen, Fragen, Antworten, Versionen, Budgetwerte, URLs, Konten, Statuswerte, Quellen und Freigaben dürfen nicht fest in Produktansichten übernommen werden. Echte Daten und bestehende Zustandsmodelle verwenden.
- Die kleinen Mockup-Dialoge vereinfachen den Funktionsumfang. Aktuelle Felder, Optionen, Validierungen, Aktionen, Plattformunterschiede und verwaltete Vorgaben vollständig erhalten, auch wenn sie in der Vorschau fehlen.
- Keine Simulation, Timer-Erfolgsmeldung, optimistische „fertig“-Markierung oder lokale Demo-Zustandsmaschine anstelle realer Rückmeldungen übernehmen. Kein externes Icon-CDN, keine Vorschau-Hilfen wie `Tweak`/`window.openai`, keine Aufweichung von CSP/Capabilities.
- Der aktuelle Produktcode und dokumentierte Verträge bestimmen Funktionen und Berechtigungen. Die Referenz bestimmt Hierarchie, Abstände, Flächen, Typografie und Gewichtung. Bei einem Konflikt die Gestaltung an den echten Funktionsumfang anpassen.

## Gemeinsame Designvorgaben

### Flächen und Farben

Semantische CSS-Variablen verwenden und vorhandene Tokens weiterentwickeln. Die Namen unten sind Rollen, keine Pflicht zum Umbenennen bestehender Variablen. Systemdarstellung bzw. vorhandene Theme-Einstellung beibehalten; beide Themes gleichwertig gestalten. Keine zusätzliche Theme-Einstellung nur deshalb einführen, weil die Vorschau einen Umschalter enthält.

| Rolle | Hell | Dunkel |
|---|---|---|
| Hauptfläche | `#fcfcfe` | `#17191f` |
| Sidebar / zurückgesetzte Fläche | `#f3f4f8` | `#111319` |
| Karten / Eingaben / Dialoge | `#ffffff` | `#20232c` |
| Primärer Text | `#1b2032` | `#f0f2f9` |
| Sekundärer Text | `#656b7c` | `#a3aabc` |
| Trennlinien | `#e3e6ee` | `#303440` |
| Hover / neutrale Hervorhebung | `#eceef5` | `#292d39` |
| Erfolg | `#18785b` | `#6bd4a7` |
| Hinweis / Aufmerksamkeit | `#9c5d05` | `#edbf6b` |
| Fehler | `#b53b47` | `#f09096` |

App-Akzent siehe unten. Aktive Navigation und dezente Hervorhebungen erhalten etwa 10 % Akzent auf der jeweiligen Fläche. Primärbuttons nutzen den vollen Akzent, mit weißer Beschriftung im hellen Theme und sehr dunkler Beschriftung auf dem helleren Dunkelmodus-Akzent; Kontrast am tatsächlichen Hintergrund prüfen. Farben ergänzen Text und Symbole, ersetzen sie nicht. Keine großflächigen dekorativen Verläufe oder neue Markenlogos; vorhandene Produktassets weiterverwenden.

### Typografie und Geometrie

- Systemschrift: `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `system-ui`, `sans-serif`; keine Webfont-Abhängigkeit.
- Fließtext 14 px, längere Antworten 14–15 px mit etwa 1,65 Zeilenhöhe. Sekundärtext 12 px; 11 px nur für wenig wichtige Metadaten, nicht für zentrale Aktionen oder Fehlermeldungen.
- Seitentitel 26–29 px, Gewicht etwa 650–700, dezentes negatives Letterspacing; Abschnittstitel 18 px, Elementüberschriften 14 px/600–650. Keine große Marketingüberschrift zulasten von Inhalt; Seitentitel bleiben fachlich verständlich.
- Abstandsraster 4/8/12/16/24/32 px. Inhaltsränder meist 24–28 px, auf schmalen Fenstern 16 px. Verwandte Elemente enger gruppieren als unabhängige Abschnitte.
- Buttons und Eingaben Radius 8 px; Karten 12 px; Composer und Dialoge 12–16 px. Fensterrahmen bleiben nativ. Schatten sparsam für schwebende Flächen und Dialoge, keine Schattenkachel um jeden Inhalt.
- Reguläre Controls ungefähr 36 px hoch; verständliche Beschriftungen, ausreichende Klickflächen und sichtbarer Tastaturfokus. Wichtige Aktionen nicht ausschließlich als unbeschriftete Icons anbieten.
- Eine dominante Primäraktion pro Ansicht. Nebenaktionen als neutrale Buttons oder beschriftetes Menü; destruktive Aktionen eindeutig unterscheiden. Kontextaktionen beim betroffenen Objekt halten.
- Tabellen mit ruhigen horizontalen Trennlinien und klaren Zeilen; keine unnötigen Dashboard-Kennzahlen. Lange Namen und Pfade dürfen Aktionen nicht verdrängen.

### Verhalten, Zugänglichkeit und Qualität

Alle bestehenden Zustände gestalten: Einrichtung, Anmeldung/Abmeldung, leer, lädt, erfolgreich, fehlgeschlagen, abgebrochen, gesperrt/verwaltet und veraltete/unbekannte Daten. Bei deaktivierten wichtigen Aktionen den Grund zugänglich erklären. Wesentliche Information nicht in reine Hover-Tooltips verschieben.

Native Controls, sinnvolle Labels, Fokusreihenfolge, Escape und Fokus-Rückgabe bei Dialogen erhalten. Statuswechsel angemessen ankündigen; nicht jedes gestreamte Token vorlesen. Reduzierte Bewegung respektieren. Bestehende Sprachen und Übersetzungsmechanismen vollständig mitziehen.

Inhalt bleibt bei Fensteränderungen stabil: keine verlorene Eingabe, springende Textauswahl, gestohlener Fokus, neu startende Animation oder verlorene Scrollposition durch unnötigen DOM-Neuaufbau. Gestreamte Antworten und Logausgaben dürfen die Oberfläche nicht blockieren. Desktop-Fenster bei Standard- und Mindestgröße sowie vergrößerter Schrift prüfen; nicht das Vorschau-Layout starr skalieren.

## OpenReleasePilot-Repository: zwei Apps, eine gemeinsame Umsetzung

**Zielrepo:** `OpenReleasePilot`. Es enthält **OpenReleasePilot und OpenSitePilot**. Beide Apps müssen nach der Umstellung funktionieren; es gibt kein zweites OpenSitePilot-Repository für diesen Auftrag. Die Apps behalten getrennte Aufgaben, native Kommandos, Konfigurationen und Daten. Gemeinsame Darstellung in den bereits gemeinsam genutzten Dateien umsetzen.

**Akzente:** OpenReleasePilot hell `#5256d7`, dunkel `#a7aaff`; OpenSitePilot hell `#087d7b`, dunkel `#58d8cb`. Den Akzent pro App-Shell setzen, nicht globale Farben zwischen beiden Apps überschreiben. Aktuelle Produktassets `desktop/ui/icon.svg` und `desktop/ui/site-icon.svg` erhalten.

### Einstieg und betroffene Dateien

| Zuständigkeit | Datei |
|---|---|
| Aktuelle Produktgrenzen / Entwicklung | [README](../../README.md) |
| OpenReleasePilot | [releasepilot.js](../../desktop/ui/releasepilot.js) |
| OpenSitePilot | [sitepilot.js](../../desktop/ui/sitepilot.js), [site.html](../../desktop/ui/site.html) |
| Gemeinsame Controls, Dialoge, Design | [common.js](../../desktop/ui/common.js), [style.css](../../desktop/ui/style.css) |
| Gemeinsamer Rust-Workspace | [Cargo.toml](../../desktop/Cargo.toml), `desktop/core/` |
| Native App-Einstiege | `desktop/src-tauri/`, `desktop/sitepilot/` |

Vorhandene `docs/ui-review/`-Unterlagen sind historische Ergänzungen, keine aktuelle Funktionsquelle: der dortige Review bezieht sich auf Version 0.3.4, `ui/app.js` und teils einen Website-Bereich innerhalb ReleasePilot. Maßgeblich sind der aktuelle Code, das aktuelle README und die heutige Trennung in zwei Apps. Diesen alten Review nicht als zusätzlichen Implementierungsauftrag übernehmen.

## A. OpenReleasePilot

**Referenz:** OpenReleasePilot → Repositories, Repository-Detail, Aufträge und konkrete Freigabe.

1. **Navigation:** „Repositories“ und „Aufträge“, Einstellungen weiterhin erreichbar. Aktuelle Forge-Auswahl (GitHub/GitLab-Instanzen), Repository-Verwaltung, Auswahl und Filter erhalten. Die Mockup-GitHub-Instanz ist nur ein Beispiel; keine vorhandene Forge ausblenden. Kein Website-Bereich neu hinzufügen.
2. **Repository-Übersicht:** Name und Zustand stehen zuerst, darunter kurzer tatsächlicher Befund. Trennlinien statt eines unruhigen Kachelrasters; eine kompakte Liste/Tabelle ist zulässig, solange Auswahl, Verwaltung und vorhandene Aktionen erhalten bleiben. „Pflegen …“ als Primäraktion für die reale Auswahl. Nicht selektierte Repositories nicht stillschweigend in einen Auftrag übernehmen. Lange Namen, leere Filterergebnisse und viele Repositories berücksichtigen.
3. **Zustände:** Health-Befund und Auftragsfortschritt nicht zusammenwerfen. „In Ordnung“ nur bei erfolgreicher aktueller Prüfung; unbekannt, veraltet, API-Fehler und CI-Fehler separat abbilden. Nicht jedes Repository bekommt allein aus Farbkonsistenz einen Erfolgsstatus.
4. **Repository-Detail:** PR/MR, CI, Release-Regeln, Inhaltsziele, Repository-Einstellungen und aktuelle Verwaltungsaktionen vollständig erhalten. README-/Wiki-Inhaltsziele sind weiterhin Teil von ReleasePilot. Die vereinfachte Mockup-Detailansicht ist kein neuer reduzierter Funktionsumfang.
5. **Aufträge:** eindeutiger Zielkontext, tatsächlich laufender Schritt, Blocker, Ergebnis und nächste Entscheidung. Vorhandene Zusammenfassung, Pflegeplan, Logs, Recovery, Abbruch und Abschließen erhalten. Fortschritt ausschließlich aus echten Zuständen ableiten, keine künstlichen vier Phasen erzwingen, falls der echte Auftrag anders aufgebaut ist.
6. **Freigabe:** konkreten Vorschlag mit Repository/Ziel, Art der Änderung, relevanter Version/Commit/PR und zugänglichem Diff zeigen. Freigabe, Ablehnung und Fortsetzung müssen die vorhandenen Kommandos und Sicherheitsprüfungen nutzen. Ein klar beschrifteter Button zur tatsächlichen Aktion; die Mockup-Beschriftung „Freigabe simulieren“ gehört nicht in die App. Freigaben werden bei geänderter Grundlage weiterhin ungültig. Eine Merge-Freigabe ist keine Release-/Tag-Freigabe.
7. **Einstellungen / Formulare:** vollständige Forge-, Repository-, Release-, Inhaltsziel-, Modell-/Budget- und sonstige vorhandene Optionen in dieselbe Designsprache bringen. Kein Mockup-Dialog darf existierende fachliche Auswahl ersetzen.

**Erhalten:** ein offener Auftrag pro Ziel und bestehende globale Ausführungssperre; Abbruch und Wiederaufnahme; konkrete Freigabe externer Änderungen; Tags/Releases nur mit vorhandener Regel und gültiger CI auf dem exakten Commit; bestehende Git-/Credential-/Signatur-/Pfadschutzregeln. Keine externen Änderungen aus bloßem Öffnen einer neu gestalteten Ansicht auslösen.

## B. OpenSitePilot

**Referenz:** OpenSitePilot → Website, vier Schritte, Produktauswahl, Ergebnis/Protokoll, Einstellungen.

1. **App-Rahmen:** eine Website mit Navigation „Website“ und Einstellungen. Website-Name und Veröffentlichungsziel klar benennen. Keine erfundene Website-Flotte oder direkte Verbindung zu ReleasePilot einführen. Schließen beendet diese App; kein Tray-Verhalten von den anderen Apps übernehmen.
2. **Ablauf:** „Prüfen → Inhalte aktualisieren → Veröffentlichen → Live-Seite prüfen“ als gut lesbare Schrittfolge mit ausgewähltem Detailbereich. Schritte bleiben einzeln startbar. „Aktualisieren und veröffentlichen“ bleibt die eine dominante Gesamtaktion; Einzelaktionen erhalten geringeres visuelles Gewicht. Erklärung der Folgen an der jeweiligen Aktion erhalten.
3. **Schrittzustände:** ausgewählt, bereit, läuft, erfolgreich, Warnung, fehlgeschlagen und veraltet unterscheidbar anzeigen. Erfolg beim Veröffentlichen darf nicht die Live-Prüfung als erledigt markieren. Ein später einzeln gestarteter Schritt darf ungestartete frühere Schritte nicht als erfolgreich färben. Ergebnis und Log beziehen sich immer auf den ausgewählten Schritt.
4. **Produkte:** Checkbox, Produktname, dokumentierte/aktuelle Version und belastbarer Status. Nur ausgewählte Produkte prüfen/aktualisieren. Versionsdaten nach tatsächlicher Inhaltsaktualisierung neu lesen; kein „aktuell“ aufgrund eines Buttonklicks. Fehler beim Release-Abruf bedeutet unbekannt und blockiert den bestehenden abhängigen Ablauf, nicht „kein Update“.
5. **Aufgaben:** aktuelle Update-Aufgaben, Reihenfolge, Modell-/Effort-Vorgaben, zusätzliche Anweisung und technische Details erhalten. Seltener benötigte Details dürfen aufklappbar werden; wichtige Sperren oder Folgen bleiben sichtbar. Eingaben und Logposition während Updates stabil halten.
6. **Veröffentlichung:** Wirkung verständlich nennen: Build, Repository-Push, inkrementeller SFTP-Upload einschließlich bestehender Löschlogik. Gesamtaktion läuft bis zur Live-Prüfung und stoppt bei Fehlern. Keine zusätzliche Pflichtbestätigung allein aus dem Mockup ableiten; vorhandene Freigabe-/Bestätigungspolitik und die ausdrückliche Semantik der Gesamtaktion erhalten. Der Mockup-Dialog diente nur der Demonstration.
7. **Produkt hinzufügen:** aktuelle Repository-Auswahl und Erstellen/Build/Commit-Abläufe erhalten. Erstellung einer Produktseite ist keine Veröffentlichung. Auswahl von Website-Produkten bleibt getrennt von verwalteten ReleasePilot-Repositories.
8. **Einstellungen:** Website-Repository, lokale Ordner, SFTP-Ziel, Host-Key-Bestätigung, sichere Zugangsdaten, Updates und vorhandene Migration vollständig erhalten. Keine echten Passwörter/Tokens in JavaScript-Persistenz oder Logs. Zugriff auf GitHub weiterhin über den vorhandenen CLI-Login; kein neues Token-Feld erfinden.

**Erhalten:** sichere Git-/Prozess-/SFTP-Abläufe, Upload-Manifest, Host-Key-Vertrauen, Fehlerstopp, Abbruch, Auswahlgrenzen und echte Live-Datei-/Hashprüfung. Keine Umgehung von Prüfungen, um eine schöne grüne Schrittfolge zu erzielen.

## Gemeinsame technische Umsetzung und Prüfung

`desktop/ui/common.js` und `desktop/ui/style.css` werden von beiden Apps genutzt. App-spezifische Klassen/Tokens klar begrenzen; gemeinsame Controls nur einmal anpassen. Bestehende SVG-/Icon-Helfer und lokale Assets verwenden. Die bewusst unveränderte macOS-Mindestversion und bestehende Paket-/Signierkonfiguration nicht im Rahmen dieses Designs ändern.

Vom Repository-Root Syntax nur der geänderten Dateien prüfen, beispielsweise:

```sh
node --input-type=module --check < desktop/ui/releasepilot.js
node --input-type=module --check < desktop/ui/sitepilot.js
node --input-type=module --check < desktop/ui/common.js
```

Die Apps im vorhandenen Setup **jeweils ausdrücklich** starten; `desktop/` enthält mehrere Tauri-Apps:

```sh
# Aus dem Repository-Root, jeweils separat:
cd desktop/src-tauri
npx tauri dev
```

```sh
# Aus dem Repository-Root, jeweils separat:
cd desktop/sitepilot
npx tauri dev
```

Bei Rust-Änderungen zunächst passende vorhandene Tests im betroffenen Workspace-Paket auswählen. Nur bei geänderten gemeinsamen Grenzen ist `cargo test --locked --workspace` aus `desktop/` sinnvoll. Ignorierte Integrationstests nicht routinemäßig aktivieren: sie können echte GitHub-/Claude-Aufrufe ausführen. Keine Pakete nur für Zusatzchecks installieren und keine nicht vorhandenen `npm test`-/Lint-/Typecheck-Aufrufe erfinden. Installer nur bei Packaging-Änderungen bauen, dann die existierenden app-spezifischen Skripte verwenden.

Abnahme für **beide Apps**: Hell/Dunkel, Standardfenster 1080 × 800 und bisheriges Minimum 860 × 560, lange Inhalte, leere/volle Listen, Filter/Auswahl, Tastatur/Modalfokus, Dialogfehler, laufende/abgebrochene/fehlgeschlagene Vorgänge und stabile Eingaben/Logs. Zusätzlich ReleasePilot: mehrere Forges, stale/unknown Health, gesperrte Ziele, veraltete Freigabe, CI-Fehler, konkrete Zielauswahl. Zusätzlich SitePilot: abgewählte Produkte, fehlende SFTP-Konfiguration, unbekanntes Release, Einzelschritte, Fehlerstopp im Gesamtlauf, Upload ohne Live-Prüfung und separate Verifikation. Mit synthetischen Antworten/isolierten Fixtures arbeiten; keine realen Repositories mergen/taggen/pushen oder Websites veröffentlichen, um das Design zu testen.

Aktuelle Distribution/CI dieser beiden Apps zielt auf macOS; Windows ist im README als ungetestet beschrieben. Aus dem plattformübergreifenden Mockup keine neue Windows-Unterstützungszusage oder zusätzlichen Portierungsauftrag ableiten.

## Arbeitsfolge und Fertigkriterien

1. Aktuellen UI-Zustand und native Fenstergrenzen anhand der genannten Dateien erfassen. Die Referenz mit realen Funktionen abgleichen; keine erneute Vollrepo-Analyse oder Roadmap anlegen.
2. Lokale Designtokens und bestehende Controls anpassen, dann die spezifischen Hauptansichten, Nebenansichten und vollständigen Dialoge. Bestehende Daten-/Command-Schnittstellen verwenden. Falls nur die Darstellung betroffen ist, Rust unverändert lassen.
3. Relevante Zustände, Interaktionen und beide Themes prüfen. Für tatsächliche Verhaltensänderungen fokussierte Regressionstests hinzufügen; keine Tests, die nur CSS-Werte oder diese Anleitung nacherzählen. Syntaxprüfung und Mockup-Screenshots sind kein Nachweis nativer Funktion.
4. Den finalen Diff auf unbeabsichtigte Funktionsverluste und abgeschwächte Guards prüfen. Nur wenn die Umsetzung dokumentiertes Verhalten verändert, bestehende passende Dokumentation aktualisieren.
5. Knapp berichten: umgesetzte Ansichten, wichtige Dateien, tatsächlich geprüfte Plattformen/Interaktionen, relevante Screenshots und verbleibende Blocker. Ungeprüfte native Integration ausdrücklich von geprüftem Browser-Rendering unterscheiden. Keine Umsetzung als fertig melden, wenn wichtige vorhandene Dialoge oder Zustände nur Platzhalter sind.

**Fertig ist die Aufgabe, wenn** die betroffenen vollständigen Oberflächen erkennbar der Studio-Familie entsprechen, alle bisherigen Funktionen und Schutzmechanismen erhalten sind, zentrale Abläufe und Fehlerzustände geprüft wurden und die Umsetzung mit einem fokussierten Diff reviewbar ist. Danach stoppen; kein anschließendes Refactoring oder weiterer Produktumfang.
