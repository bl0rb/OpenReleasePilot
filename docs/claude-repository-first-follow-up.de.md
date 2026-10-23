# Folgeanweisung für Claude: Repository-Pflege mit optionaler Veröffentlichung

Stand: 12. Oktober 2026. Ausgangsbasis ist OpenReleasePilot 0.3.0, geprüft am Commit `be89dc1`.

## 1. Auftrag

Entwickle OpenReleasePilot auf Basis der vorhandenen Version 0.3.0 zu einer Anwendung für die automatisierte Pflege ausdrücklich ausgewählter Repositories und ihrer zugehörigen Inhalte weiter.

**Die verwalteten Repositories bilden die Grundlage. Eine Website ist ein optionales nachgelagertes Ziel und gegebenenfalls der letzte Veröffentlichungsschritt.**

Der Nutzer beschreibt sein Ziel so:

> Ich möchte für meine angegebenen Repositories nur noch die notwendigen Entscheidungen treffen. Die App soll die Arbeit erledigen, die ich bisher für Updates manuell über Claude oder VS Code steuern muss.

Die App übernimmt Zustandsprüfung, Planung, Claude-Aufrufe, technische Ausführung, Tests, Fortschritt und Ergebnisprüfung. Der Nutzer entscheidet über fachlichen Umfang, konkrete Vorschläge und Freigaben. Eine bloße Folge von Links zu externen Werkzeugen erfüllt diesen Auftrag nicht.

Diese Folgeanweisung löst die frühere Beschränkung auf die Produkt-Repositories einer Website bewusst ab. Die Sicherheitsprinzipien bleiben erhalten. Der bereits implementierte Repositories-Bereich, Health-Check und die Pflegefunktion werden weiterverwendet, nicht erneut gebaut.

## 2. Produktmodell

Das Grundmodell lautet:

```text
Verwaltetes Repository
├── Zustand und offene Aufgaben
├── Repository-Pflege
├── Release-Regeln, falls benötigt
└── Zugeordnete Inhalte und Veröffentlichungsziele: keines, eines oder mehrere
    ├── README / Dokumentationsdateien
    ├── Wiki
    └── Website
```

Ein Repository muss ohne Website, SFTP-Zugang, Website-Build und veröffentlichtes Release verwaltet und gepflegt werden können. Fehlende optionale Ziele sind kein Einrichtungsfehler.

Ein Website-Repository kann selbst ein verwaltetes Repository sein und zugleich als Inhaltsziel dienen. Mehrere Repositories können dieselbe Website speisen. Diese Beziehungen dürfen weder doppelte Pflegeaktionen noch rekursive Abläufe auslösen.

„Generisch“ bedeutet hier: unabhängig von einer Website und für verschiedene Pflegeziele geeignet. Es bedeutet nicht, jetzt beliebige Git-Anbieter, ein Plugin-System oder eine frei programmierbare Workflow-Engine zu implementieren. GitHub, die vorhandene Claude-Code-Integration und bestehendes SFTP bleiben die ersten unterstützten Integrationen.

## 3. Repository-Verwaltung von Website-Konfiguration lösen

Führe eine eigenständige Liste verwalteter Repositories in der App-Konfiguration ein. Nutze die vorhandene GitHub-Auswahl und Clone-Erkennung.

- Repository explizit aufnehmen, bearbeiten, deaktivieren und aus der Verwaltung entfernen können.
- Entfernen aus der Verwaltung löscht weder das Remote-Repository noch lokale Dateien oder fremde Konfiguration.
- Repository-Identität mindestens anhand des normalisierten `owner/name` deduplizieren; verfügbare GitHub-IDs berücksichtigen.
- Lokale Kopie optional zuordnen. GitHub-basierte Statusprüfung und bestehende PR-Pflege bleiben ohne Clone möglich.
- Pflegeauswahl direkt im Repositories-Bereich anbieten: Einzelrepository und mehrere explizit ausgewählte Repositories.
- Website-Produktauswahl und Repository-Pflegeauswahl unabhängig halten.
- Ohne Website muss der Einstieg lauten: „Repositories auswählen“, nicht „Website-Projekt und SFTP einrichten“.
- Zugangsdaten und Werkzeuge erst für die betreffende Aktion voraussetzen. Ein lesender Statuscheck braucht keine Claude-Sitzung; README-Pflege braucht keinen SFTP-Zugang.

`repo_targets` und vergleichbare Einstiege dürfen nicht mehr von einer erfolgreich geladenen `Site` abhängen. `Site` wird eine optionale Konfiguration für ein Website-Ziel.

## 4. Inhaltsziele ausdrücklich modellieren

Ein Pflegeziel beschreibt, **welche Inhalte wo und auf welcher Grundlage aktualisiert werden sollen**. Trenne das Quellrepository vom Ziel, auch wenn beide identisch sind.

Pro Ziel werden benötigt:

- stabile Identität, Typ und verständlicher Name;
- zugeordnete Quellrepositories;
- Zielrepository bzw. Wiki-Remote, erlaubte Pfade und Zielbranch, soweit anwendbar;
- fachlicher Auftrag und verwendeter Skill;
- Quellbasis: bestätigtes Release oder festgehaltener Repository-Commit;
- geeignete Prüfungen und Veröffentlichungsweg;
- letzter erfolgreich aktualisierter und letzter veröffentlichter Stand.

„Aktuell“ gilt immer für ein bestimmtes Ziel und eine bestimmte Quellrevision. Ein einziger `documented`-Wert darf nicht gleichzeitig Website, README und Wiki als erledigt markieren. Ebenso sind „Änderung vorbereitet“, „PR erstellt“, „gemergt“ und „veröffentlicht“ verschiedene Ergebnisse.

### README und Dokumentationsdateien

Ermögliche die Aktualisierung einer README oder ausdrücklich ausgewählter Dokumentationsdateien im Produktrepository oder einem zugeordneten Dokumentationsrepository.

```text
Quellstand lesen → Änderungen in isoliertem Branch vorbereiten
→ passende Prüfungen → Änderungsvorschau → Freigabe → Pull Request
```

Kein direktes Schreiben auf den Default Branch des Produktrepositorys. Ein erstellter PR ist noch keine veröffentlichte Dokumentationsänderung. Seine weitere Bearbeitung folgt der vorhandenen sicheren PR-Pflege oder der ausdrücklich gewählten Freigabe.

### Wiki

Unterstütze ein klar definiertes erstes Wiki-Ziel, beispielsweise ein GitHub-Wiki. Es besitzt ein eigenes Remote und einen eigenen Veröffentlichungsweg; nicht automatisch wie ein Produkt-PR behandeln.

Änderungen zunächst isoliert vorbereiten, prüfen und als konkrete Vorschau zeigen. Wenn das Ziel keinen PR-Ablauf anbietet, Veröffentlichung nur über den ausdrücklich bestätigten, für dieses Ziel vorgesehenen Weg ausführen. Vor dem Push Remote-Stand erneut prüfen; bei konkurrierenden Änderungen stoppen, niemals automatisch per Force-Push überschreiben.

### Website

Verwende den vorhandenen Website-Workflow als optionalen letzten Veröffentlichungsbaustein:

```text
Website-Inhalte aktualisieren → Build → Commit/Push → Upload → Verify
```

Die bisherigen Aufgaben/Skills, SFTP-Unterstützung und Dateiverifikation erhalten. URL, Build, Ausgabeordner und SFTP-Konfiguration gehören ausschließlich zu diesem Ziel. Ohne Website-Ziel bleibt der übrige Pflegeauftrag vollständig nutzbar.

## 5. Einen zusammenhängenden Pflegeauftrag schaffen

Der Nutzer wählt Repositories und gewünschte Tätigkeiten. Die App stellt daraus einen konkreten Ablauf zusammen. Keine Pflicht, alle Stufen immer auszuführen.

```text
Repositories auswählen
→ Status und Handlungsbedarf prüfen
→ Pflegeplan mit nötigen Entscheidungen vorlegen
→ freigegebene Repository-Pflege ausführen
→ optional Release durchführen und Veröffentlichung bestätigen
→ ausgewählte README-/Wiki-/Dokumentationsänderungen vorbereiten
→ nötige Inhaltsfreigaben einholen und zielgerecht abschließen
→ optional Website aktualisieren, veröffentlichen und verifizieren
```

Verbindliche Abgrenzungen:

- PR-Pflege, Cleanup und Release getrennt auswählbar machen. „PRs pflegen“ darf nicht zwangsläufig einen Patch-Tag erzeugen.
- Die vorhandene sichere Patch-Tag-Planung erhalten. Bei neu aufgenommenen Repositories Release-Automatik erst nach eindeutiger Konfiguration aktivieren; keine Versionierungsregeln erraten.
- Ein Tag ist kein veröffentlichtes Release. Benötigt ein Folgeschritt ein Release, auf das erfolgreiche, zum erwarteten Tag gehörende veröffentlichte Release warten. Wartezeit begrenzen, Fortschritt und erforderliche Aktion anzeigen.
- Dokumentation kann auch ohne neues Release aktualisiert werden. Die Aufgabe verwendet dann den ausdrücklich gewählten Commit als Basis und darf nicht wegen „keine neue Version“ pauschal entfallen.
- Ein Release startet weder automatisch alle Inhaltsziele noch zwingend ein Website-Deployment. Maßgeblich sind der ausgewählte Auftrag und seine Freigaben.
- Bei einer gemeinsam genutzten Website Änderungen mehrerer Quellrepositories gesammelt vorbereiten. Keine konkurrierenden Schreibläufe im selben Ziel. Die tatsächlich verwendeten Quellstände sichtbar halten.
- Fehler stoppen abhängige Schritte. Unabhängige Teile dürfen mit ihren eigenen Ergebnissen fortfahren; ein Teilerfolg darf nicht als vollständig erfolgreicher Auftrag erscheinen.

Der Nutzer muss innerhalb eines freigegebenen Ablaufs nicht jeden technischen Befehl einzeln starten.

## 6. Freigaben auf Entscheidungen konzentrieren

Die App soll pro Repository verständlich zeigen:

- Was wurde gefunden?
- Was schlägt die App vor, und warum?
- Was kann sie innerhalb des gewählten Auftrags selbst erledigen?
- Welche Entscheidung oder Freigabe fehlt?
- Was ist bereits erledigt und was folgt danach?

Beispiele: „3 geprüfte Updates übernehmen“, „Major-Update zurückstellen“, „Dokumentationsänderungen freigeben“, „Website veröffentlichen“.

Leseprüfungen und lokale Vorbereitung im ausdrücklich gestarteten Auftrag können ohne einzelne Zwischenfragen laufen. Vor externen Änderungen bleiben eine konkrete Vorschau und passende Freigabe erforderlich. Eine gespeicherte Zuordnung, ein Health-Befund oder ein Claude-Verdict ersetzt keine Freigabe.

Freigaben fachlich bündeln, etwa für einen geprüften Pflegeplan. Änderungen an README/Wiki/Website müssen in der App als konkrete Änderungsvorschau beurteilbar sein; der Nutzer soll dafür nicht in VS Code wechseln müssen. Neu erzeugte Inhalte gelten nicht schon deshalb als freigegeben, weil vorher ein PR-Merge bestätigt wurde.

Eine fokussierte Vorschau der von der App erzeugten Änderungen genügt; keinen allgemeinen Git-Client bauen. Wenn sich Umfang, Ziel oder relevante Quellrevision ändern, den betroffenen Vorschlag aktualisieren und nötigenfalls erneut vorlegen. Unveränderte, bereits freigegebene Schritte nicht grundlos wieder zur Entscheidung stellen.

Zeitgesteuerte oder dauerhaft unbeaufsichtigte Schreibläufe sind nicht Voraussetzung dieses Auftrags. Zuerst muss der vom Nutzer gestartete Ablauf vollständig und zuverlässig funktionieren.

## 7. Navigation und Zustände

**Repositories wird die Startansicht.** Zeige vorrangig offene Entscheidungen und den nächsten sinnvollen Schritt; technische Details bleiben erreichbar.

```text
ARBEIT
  Repositories
    Status und Pull Requests
    Pflege und Release
    Zugeordnete Inhalte: README, Wiki, Website
    Aktueller Auftrag und Ergebnisse
  Läufe
    Aktive und bisherige Pflegeaufträge

EINRICHTUNG
  Repositories und Zuordnungen
  Zugänge und Ausführungsregeln
```

Das ist eine fachliche Gliederung, keine Vorgabe für eine große Sidebar. Tabs und bestehende Detailbereiche wiederverwenden. Den vorhandenen Website-Ablauf über das zugeordnete Ziel erreichbar halten; ein vorhandener direkter Website-Einstieg darf als Abkürzung bleiben.

Repository-Health und Auftragsfortschritt getrennt darstellen. Ein Repository kann Handlungsbedarf besitzen, während die App bereits daran arbeitet.

Für Aufträge mindestens unterscheiden: geplant, in Vorbereitung, Freigabe nötig, läuft, wartet auf CI/Release/PR, erfolgreich, teilweise erfolgreich, blockiert, fehlgeschlagen und abgebrochen. Immer den Grund und eine passende nächste Aktion zeigen. Fehlende optionale Website-Einrichtung darf keine globale Warnung erzeugen.

## 8. Aufträge speichern und sicher fortsetzen

Speichere Auftrag, Ziele, festgehaltene Quellrevisionen, Planrevision, Freigabestand, einzelne Ergebnisse und offene Schritte dauerhaft. Atomar geschriebene Dateien genügen, solange sie die Anforderungen zuverlässig erfüllen; keine Datenbank nur dafür einführen.

- App-Neustart darf weder erledigte Schritte vergessen noch einen Auftrag still neu starten.
- Fortsetzen beginnt mit Revalidation. Gespeicherte Freigaben gelten nur für unveränderten Umfang und weiterhin belegbare Voraussetzungen.
- Einen unterbrochenen Schreibschritt zuerst anhand des Remote-Zustands auflösen; nicht blind erneut mergen, kommentieren, taggen, PRs erstellen oder veröffentlichen.
- PRs, Tags, Zielcommits und Veröffentlichungsergebnisse einem Auftrag eindeutig zuordnen.
- Lokale Arbeitsstände und vom Nutzer vorgenommene Änderungen schützen. Keine unbeaufsichtigten Resets, Force-Pushes oder Bereinigungen fremder Änderungen.
- Abbruch beendet weitere Schritte, erhält Teilergebnisse und behauptet keinen Rollback.
- Für den ersten Ausbau die vorhandene zentrale Sperre für Schreibaufträge beibehalten. Insbesondere dasselbe Ziel nie parallel verändern.
- Keine Tokens oder Passwörter in Auftrag, Vorschau oder Logs speichern.

## 9. Bestehende Sicherheit erhalten und Restfehler schließen

Die in 0.3.0 umgesetzten Regeln sind die Grundlage: rein lesende Health-/Pflegeprüfung, serverseitige Pläne, eindeutige gültige Reviews, erneute Prüfung vor jeder Mutation, Tags nur mit bestätigter CI des Zielcommits sowie optionales geschütztes Cleanup.

Green und nach bestehenden Regeln begründetes Yellow bleiben zulässig; Red oder fehlende/ungültige Bewertungen erlauben keinen Merge. Pflege umfasst weiterhin passende PRs von Menschen und anderen Bots, nicht nur Dependabot.

Bei der Prüfung von 0.3.0 wurden diese Restfehler festgestellt. Prüfe sie am aktuellen Stand und behebe sie, bevor die neuen Abläufe darauf aufbauen:

1. **Lokales Cleanup:** `Proof::Contained` prüft teilweise nur gegen ein veraltetes lokales `origin/<default>`. Vor dem Löschen den aktuellen Remote-Stand und den gültigen Merge-Nachweis bestätigen. Bei fehlgeschlagenem oder unzureichendem Abgleich nicht löschen.
2. **Release-Abfragen:** `find_outdated` transportiert Abfragefehler nicht als unvollständige Entscheidungsgrundlage. Eine leere Ergebnisliste kann dadurch „alles aktuell“ bedeuten, obwohl der Stand unbekannt ist. Fehler, keine Veröffentlichung und keine neuere Version getrennt abbilden; abhängiges automatisches Veröffentlichen bei ungeklärtem Stand stoppen.
3. **Tag nach CI-Wartezeit:** Die Release-Linie unmittelbar vor dem Schreiben erneut prüfen. Ein inzwischen höherer Tag oder eine verschobene Vergleichsbasis macht die alte Tag-Planung ungültig, auch wenn der Default-Branch unverändert ist.

Die neue Inhaltsbearbeitung erhält eigene begrenzte Schreibrechte; die bestehende lesende PR-Review-Umgebung nicht aufweiten. Zielrepository, erlaubte Pfade, erwarteter Ausgangscommit und geplanter Veröffentlichungsweg werden durch Anwendungscode geprüft, nicht nur im Claude-Prompt beschrieben. Ausgaben und referenzierte Repository-Inhalte dürfen diese Grenzen nicht verändern.

## 10. Kompatibilität und Architektur

Baue schrittweise auf der vorhandenen Rust-/Tauri-/JavaScript-Struktur auf:

| Bestehender Bereich | Weiterverwendung / Änderung |
| --- | --- |
| `desktop/core/src/repos.rs` | Identität, Fakten und Health erhalten; Ziele unabhängig von `Site` aufbauen. |
| `desktop/core/src/repocare.rs` | Sichere Pflege weiterverwenden, Restfehler beheben, Pflege/Release-Optionen trennen. |
| `desktop/core/src/claude.rs` | Lesendes PR-Review erhalten; gezielte Inhaltsaufträge separat formulieren und begrenzen. |
| `desktop/core/src/config.rs`, `site.rs` | Eigenständige Repository-Verwaltung; `Site` als optionale Website-Konfiguration behandeln. |
| `desktop/src-tauri/src/steps.rs`, `main.rs` | Wiederaufnehmbare Aufträge orchestrieren; vorhandene sichere Einzelaktionen nutzen. |
| `desktop/ui/app.js`, `style.css` | Repository-Einstieg, Entscheidungen, Inhaltsvorschau und Laufstatus ergänzen. |

Bestehende `.openreleasepilot/site.json` und der Legacy-Pfad `.openftpupdater/site.json` bleiben verwendbar. Eine vorhandene Installation muss ihren bisherigen Website-Workflow weiter ausführen können.

Vorhandene Produkt-Repositories können als kompatible Ausgangsliste übernommen werden. Neue Verwaltung und Website-Zuordnungen in der App ergänzen, ohne Website-Dateien automatisch umzuschreiben. Dedupliziert und wiederholbar übernehmen; Konflikte bei Release-Linien, Clone-Pfaden oder Zielzuordnungen sichtbar machen. Nicht unbemerkt Konfigurationswerte überschreiben.

Die bisherigen Website-Aufgaben, Produktauswahl, New product, GitHub-Auswahl, Clone-Erkennung, Build, SFTP-Upload und Verify erhalten. Die Website-Produktauswahl beeinflusst nur diesen Veröffentlichungsumfang und nicht mehr die grundsätzliche Repository-Verwaltung.

## 11. Reihenfolge der Umsetzung

1. Aktuellen Stand und vorhandene Tests kurz bestätigen; die 0.3.0-Funktionen nicht erneut implementieren.
2. Die benannten Sicherheits-/Zuverlässigkeitsfehler mit gezielten Regressionstests korrigieren.
3. Eigenständige Repository-Verwaltung und Website-unabhängigen Einstieg umsetzen.
4. Pflege-/Release-Optionen sowie persistente, fortsetzbare Aufträge ergänzen.
5. README-/Dokumentationsziel vollständig umsetzen: Vorbereitung, Prüfung, Vorschau, Freigabe, PR und Ergebnis.
6. Darauf das konkret unterstützte Wiki-Ziel mit eigenem Veröffentlichungsweg ergänzen.
7. Bestehenden Website-Ablauf als optionales letztes Ziel anbinden; gemeinsam genutzte Websites berücksichtigen.
8. Navigation, Fehlerfälle, Tests und beide READMEs auf den tatsächlichen Funktionsumfang abstimmen.

Keine Platzhalter als fertige Funktion ausgeben. Ein neues Datenmodell ohne nutzbaren Ablauf erfüllt den Auftrag ebenso wenig wie ein UI-Button, der die eigentliche Arbeit wieder dem Nutzer in einem anderen Werkzeug überlässt.

## 12. Tests und Definition of Done

Die Aufgabe ist abgeschlossen, wenn diese Szenarien funktionieren und angemessen getestet sind:

- [ ] Eine frische Installation kann ein Repository ohne Website-Konfiguration aufnehmen, prüfen und nach Freigabe pflegen.
- [ ] Die App startet mit Repositories und zeigt notwendige Entscheidungen sowie den Fortschritt automatisch ausgeführter Arbeit.
- [ ] Einzel- und Mehrfachauswahl funktionieren unabhängig von Website-Produkten; doppelte Repository-Zuordnungen erzeugen keine doppelten Aktionen.
- [ ] PR-Pflege, Cleanup und Release sind getrennt wählbar; fehlendes Release oder fehlende Website blockieren keine unabhängige Pflege.
- [ ] Eine README wird ohne Website-Build/SFTP auf festgehaltener Quelle aktualisiert, geprüft, in der App vorgeschlagen und nach Freigabe als PR bereitgestellt.
- [ ] Das unterstützte Wiki-Ziel besitzt einen funktionierenden Vorschau-/Freigabe-/Veröffentlichungsweg und erkennt konkurrierende Änderungen.
- [ ] Ein tagbasierter Ablauf wartet bei Bedarf auf das tatsächliche Release; Fehler und Timeouts führen zu einem sichtbaren, fortsetzbaren Zustand.
- [ ] Ein Auftrag kann mit seinen Teilergebnissen nach Neustart sicher fortgesetzt werden, ohne doppelte externe Aktionen.
- [ ] Inhaltsergebnisse werden pro Ziel und Quellrevision geführt. Eine aktualisierte README markiert nicht automatisch Website oder Wiki als aktuell.
- [ ] Website-Veröffentlichung bleibt optional und bildet bei Auswahl den letzten Veröffentlichungsbaustein einschließlich Verify.
- [ ] Mehrere Quellrepositories können eine gemeinsame Website konsistent aktualisieren; die neue Zuordnung löst keine Schleifen aus.
- [ ] Bestehende Website-Konfigurationen und der bisherige manuelle Website-Ablauf bleiben funktionsfähig.
- [ ] Die drei benannten Restfehler sowie veraltete Freigaben, unklare Schreibresultate, Abbruch und Zielkonflikte sind durch relevante Tests abgedeckt.
- [ ] `README.md`, `README.de.md`, Onboarding und App-Beschreibung erklären den tatsächlichen Repository-Schwerpunkt. Keine noch fehlenden Funktionen als vorhanden darstellen.

Tests mit simulierten GitHub-Antworten, temporären lokalen Repositories und kontrollierten Beispielen ausführen. Keine echten Repositories, Wikis oder Websites als automatischen Schreibtest verwenden. Netzwerk-/Claude-Tests bleiben ausdrücklich getrennt.

Im Verzeichnis `desktop` die relevanten Core- und App-Tests, `cargo check --workspace`, JavaScript-Syntaxprüfung und erforderlichen Buildprüfungen ausführen. Nicht ausgeführte Prüfungen mit Grund nennen. Zum Abschluss umgesetzte Abläufe, bewusste Verhaltensänderungen, Testergebnisse und verbleibende Grenzen berichten.

## Leitgedanke

OpenReleasePilot verwaltet eine ausdrücklich ausgewählte Repository-Flotte. Die App erkennt und erledigt die wiederkehrende Arbeit; der Nutzer trifft die nötigen Entscheidungen. README, Wiki und Website sind zugeordnete Ziele. Eine Website ist weder Voraussetzung noch Konfigurationswurzel, sondern ein optionaler letzter Schritt.
