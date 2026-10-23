# Implementierungsauftrag: Repositories als Hauptbereich

Überarbeitete Anleitung, geprüft gegen den Projektstand `a0545d4` am 11. Oktober 2026. Dieses Dokument beschreibt den Arbeitsauftrag; die Funktionen sind damit noch nicht implementiert.

## 1. Ziel und Umfang

Baue die bisherige Funktion **„Repos pflegen …“** in OpenReleasePilot zu einem permanenten Hauptbereich **„Repositories“** aus. Erhalte die vorhandene Repository-Pflege und integriere sie über dieselbe Kernlogik in die neue Oberfläche.

OpenReleasePilot soll beim Blick auf die Repository-Flotte beantworten:

> Was braucht gerade meine Aufmerksamkeit, und welche sichere Aktion kann ich als Nächstes ausführen?

Die langfristigen Produktbereiche sind **Care → Release → Publish → Verify**: Repositories pflegen, Releases vorbereiten, nachgelagerte Inhalte veröffentlichen und Ergebnisse verifizieren. Dieser Auftrag implementiert den ersten Schritt: Repository-Übersicht, lesender Health-Check, Detailansicht und Integration der bestehenden Pflege.

**Verbindlich für diesen Schritt:**

- Repositories als Hauptbereich, passend zur vorhandenen Oberfläche.
- Alle Produkt-Repositories des aktuell konfigurierten Website-Projekts anzeigen, auch bei abgewählten Produkten.
- Repository-Status, PRs, CI, Release/Tag, lokale Kopie und Website-Beziehungen anzeigen.
- Ein einzelnes Repository sowie die Repositories der ausgewählten Produkte prüfen und pflegen können.
- Die bestehende Vorschau, Ausführung, Abbruchmöglichkeit und Ergebnisanzeige erhalten.
- Die unten ausdrücklich benannten Sicherheitslücken schließen und mit Tests absichern.

**Nicht Bestandteil dieses Schritts:** freie Repository-Verwaltung über mehrere Website-Projekte, neue Codeänderungs-Automation, automatische Release-Empfehlungen, Cross-Repo-Updates, Dependency-Graphen oder ein neuer Publishing-Prozess. Diese Möglichkeiten nicht durch ein rein produktgebundenes Datenmodell verbauen, aber keine Infrastruktur dafür vorab entwickeln.

Keine allgemeine Commit-History, keinen Git-Diff-Viewer, kein Staging, keine freie Commit-Erstellung, keine vollständige Issue-Verwaltung und keinen generischen Branch-Manager hinzufügen. Kein neues Frontend-Framework und keine Komplett-Neuschreibung einführen.

## 2. Ausgangslage zuerst prüfen

Die vorhandene Architektur ist bereits in wesentlichen Teilen getrennt:

| Bereich | Bestehender Einstiegspunkt |
| --- | --- |
| Pflege: Scan, Review-Verarbeitung, Plan, Mutationen | `desktop/core/src/repocare.rs` |
| GitHub-Zugriff, Repository-Liste, veröffentlichtes Release | `desktop/core/src/github.rs` |
| Claude-Review, Regeln, Schema, erlaubte Werkzeuge | `desktop/core/src/claude.rs` |
| Lokale Clone-Erkennung | `desktop/core/src/localrepo.rs` |
| Produkte und Projektkonfiguration | `desktop/core/src/site.rs`, `config.rs` |
| Pflege-Jobs, Fortschritt, Ausführung | `desktop/src-tauri/src/steps.rs` |
| App-State und Tauri-Commands | `desktop/src-tauri/src/main.rs` |
| Oberfläche | `desktop/ui/app.js`, `style.css`, `index.html` |

Prüfe diese Stellen vor Änderungen erneut. Wiederverwenden heißt insbesondere: keine zweite Merge-, Tagging- oder Cleanup-Implementierung im Frontend oder in einem neuen Dialog aufbauen.

### Bekannte Abweichungen zwischen bisherigem Code und gewünschter Sicherheit

Diese Unterschiede sind Teil des Auftrags und dürfen nicht durch die Formulierung „bestehende Regeln unverändert“ verdeckt werden:

1. **Der bisherige Pflege-Scan ist nicht vollständig lesend.** Bei lokalem Cleanup führt `look_local` bereits `git fetch --prune` aus. Außerdem setzt `scan` teilweise Schreibrechte voraus und lehnt archivierte Repositories ab. Der neue Health-Check darf diesen Ablauf nicht unverändert aufrufen.
2. **Tagging ohne CI ist bisher möglich.** Nach einer Wartezeit ohne Checks kann `tag` trotzdem einen Tag setzen. Ziel dieses Auftrags ist strenger: Ein automatisch gesetzter Tag benötigt positiv bestätigte CI für den konkreten Zielcommit. Fehlende Checks sind kein Erfolg.
3. **Revalidation ist bisher nicht für jede Aktion vollständig.** Beispielsweise prüft `rebase` bisher nur, ob der PR noch offen ist. Die vollständigen Bedingungen in Abschnitt 9 gelten auch für bestehende Mutationen.
4. **Das Review-Schema allein reicht nicht als Merge-Schutz.** Die bisherige Verarbeitung schließt `red` aus, akzeptiert im Merge-Zweig aber nicht ausdrücklich nur `green` oder `yellow`. Ungültige oder unvollständige Bewertungen müssen im Anwendungscode sicher abgelehnt werden.

Das sind gezielte Härtungen. Sonstige Pflege- und Versionsregeln nicht nebenbei ändern; notwendige weitere Abweichungen mit Befund, Änderung und Test dokumentieren.

## 3. Repository-Identität, Auswahl und Konfiguration

**Sichtbarkeit und Ausführungsumfang sind getrennte Dinge.** Ein in der Website-Produktauswahl abgewähltes Produkt verschwindet nicht aus „Repositories“. Seine Abwahl wirkt weiterhin wie bisher auf Precheck und Update.

- Verwende eine gemeinsame Repository-Identität, für den heutigen GitHub-Support mindestens normalisiertes `owner/name`. Unterschiede in Groß-/Kleinschreibung dürfen keine Duplikate erzeugen.
- Mehrere Produkte können dasselbe Repository referenzieren. Zeige es einmal und erhalte alle zugehörigen Produkt-IDs, dokumentierten Versionen und Einstellungen.
- Repository-Name, Owner und gegebenenfalls GitHub-ID sind Daten des Repositorys; `documented` und `tagPrefix` gehören zur jeweiligen Produkt-/Release-Zuordnung.
- Bei widersprüchlichen `tagPrefix`- oder Clone-Zuordnungen nicht willkürlich den ersten Eintrag verwenden. Zeige den Konflikt und sperre die betroffene Aktion, bis ihre Grundlage eindeutig ist.
- Die vorhandene GitHub-Repository-Liste bleibt eine wiederverwendbare Auswahlquelle. Alle erreichbaren Account-Repositories automatisch als verwaltet zu übernehmen oder vollständig zu scannen, gehört nicht zum MVP.
- Die Beziehung zum Website-Projekt sichtbar machen: zugehörige Produkte und, soweit erkannt, „Website-Repository“. Das Website-Repository muss nicht zusätzlich zur verpflichtenden Produktliste als eigenständiges Pflegeziel aufgenommen werden.

Pflege muss intern eine **explizite Liste deduplizierter Repository-Ziele** annehmen. Zwei Einstiege verwenden dieselbe Logik:

1. „Dieses Repository prüfen“ aus der Detailansicht.
2. „Repositories der ausgewählten Produkte prüfen“ für den bisherigen Mehrfachablauf.

Vor dem Prüfen die tatsächlich betroffenen Repositories nennen. Eine spätere freie Mehrfachauswahl soll dieselbe Schnittstelle nutzen können. Die neue Pflegeauswahl darf die Website-Produktauswahl nicht still verändern.

Keine notwendige Migration von `site.json` und keine automatisch ergänzten Produkte. `.openreleasepilot/site.json` und der bestehende Legacy-Pfad bleiben lesbar. Zusätzliche optionale App-Einstellungen benötigen kompatible Standardwerte. Statusdaten dürfen zunächst im Arbeitsspeicher bleiben; eine Datenbank ist nicht erforderlich.

## 4. Navigation und Oberfläche

Nutze die vorhandene visuelle Sprache und die bestehende HTML-/JavaScript-/CSS-Struktur. Eine kleine Hauptnavigation oder passende Tabs reichen:

- **Overview**: bisheriger Website-Workflow mit Precheck, Update, Upload und Verify.
- **Repositories**: Übersicht und Repository-Details.
- **Settings**: vorhandene Einstellungen bleiben gut erreichbar.

Logs und Activity in vorhandene Strukturen integrieren; kein zusätzlicher Hauptbereich nur der Vollständigkeit halber. Den neuen Repositories-Bereich permanent sichtbar machen. Ohne gültiges Projekt einen hilfreichen Leerzustand zeigen. Den bisherigen Startbereich für dieses MVP beibehalten.

### Übersicht

Zeige pro Repository vorrangig:

- Name und Owner.
- Health-Zustand mit konkretem Hauptgrund.
- Offene PRs, davon Dependabot.
- CI des Default Branch.
- Letztes veröffentlichtes Release oder ausdrücklich als Tag gekennzeichnete Ersatzinformation.
- Lokale Kopie vorhanden, nicht vorhanden oder Zuordnung ungültig.
- Letzter erfolgreicher Check; veraltete oder unvollständige Daten sichtbar markieren.

Default Branch und Website-Beziehungen dürfen in einer sekundären Zeile oder aufklappbar stehen. Beispiele:

```text
OpenReleasePilot                         Aufmerksamkeit nötig
3 Dependabot-PRs können geprüft werden   Default-Branch-CI erfolgreich
Release 0.2.2 · lokale Kopie vorhanden   Zuletzt geprüft vor 2 Minuten

SomeProject                             Blockiert
Default-Branch-CI fehlgeschlagen         Details ansehen

OtherProject                            Unbekannt
GitHub-Abfrage fehlgeschlagen            Erneut prüfen
```

Die UI bleibt sprachlich konsistent mit der vorhandenen deutschen Oberfläche. Interne Enum-Namen dürfen englisch sein. Status niemals nur durch Farbe vermitteln; Text, Tastaturbedienung, sichtbarer Fokus und verständliche Button-Beschriftungen vorsehen.

### Detailansicht

**Status:** Repository, Default Branch, lokale Kopie, CI samt zugehörigem Commit, Release und Tag, Commits seit Vergleichsbasis, Website-Zuordnungen und Prüfzeitpunkt.

**Pull Requests:** Nummer/Link, Titel, Autor, Dependabot-Kennzeichnung, Zielbranch, CI, Mergeability, gegebenenfalls gültige Claude-Bewertung und geplante Aktion. Die Übersicht zählt alle offenen PRs; der Pflegeplan behandelt wie bisher die passenden PRs zum Default Branch. Diese Mengen dürfen nicht verwechselt werden.

**Maintenance:** Pflegeoptionen und Modellwahl, Prüfen, Vorschau, Ausführen, Verwerfen, Fortschritt und Ergebnis. Auch PRs von Menschen oder anderen Bots bleiben nach den bisherigen Regeln prüfbar; Pflege nicht versehentlich auf Dependabot reduzieren.

Navigation zurück zur Übersicht erhält Auswahl und Scrollposition soweit praktikabel. Ein laufender Job oder eine offene Vorschau darf durch Ansichtswechsel nicht verschwinden oder neu gestartet werden.

## 5. Statusdaten getrennt vom Pflegeplan modellieren

Ein allgemeiner Repository-Status ist **kein Maintenance-Plan**. Trenne konzeptionell:

- Repository-Identität und Beziehungen zu Produkten/Website.
- Beobachtete Fakten mit Zeitpunkten, Commit-Bezug und Datenqualität.
- Daraus abgeleiteten Health-Zustand und Gründe.
- Erlaubte Fähigkeiten bzw. gesperrte Aktionen mit Begründung.
- Einen ausdrücklich angeforderten Pflegeplan samt Ausführungsergebnis.

Dafür kleine bestehende Typen erweitern oder passende Core-Typen ergänzen; kein abstraktes Plugin-System entwickeln. Health-Entscheidungen gehören in testbare Rust-Logik, nicht verteilt in UI-Renderfunktionen.

Fehlende Daten nicht mit leeren Ergebnissen verwechseln: `0 PRs`, `keine veröffentlichten Releases`, `nicht abgefragt` und `Abfrage fehlgeschlagen` sind verschiedene Aussagen. Für wichtige Teilabfragen Erfolg/Fehler/Unvollständigkeit und den Datenstand erhalten. Ein fehlgeschlagener Refresh darf alte Daten anzeigen, aber nicht als frisch bestätigen.

CI benötigt mindestens die Zustände `passed`, `failed`, `pending`, `noChecks` und `unknown`. `noChecks` bedeutet „keine Checks gefunden“, nicht den bewiesenen Verzicht auf CI. CI immer einem konkreten Commit zuordnen, beim Repository dem aktuellen Default-Branch-HEAD. Check-Runs und Commit-Status berücksichtigen; abgeschlossene, aber unbekannte Ergebnisse nicht pauschal als Erfolg behandeln.

## 6. Health-Regeln verbindlich festlegen

Keinen numerischen Score und keine spekulativen KI-Warnungen einführen. Health beschreibt Aufmerksamkeit, nicht die Erlaubnis zum Mergen. Jede Mutation hat zusätzlich ihre eigenen Sicherheitsbedingungen.

| Zustand | Regel und Beispiele |
| --- | --- |
| **Blocked / Blockiert** | Ein aktuell belegter Repository-Blocker liegt vor: fehlgeschlagene Default-Branch-CI oder nachgewiesen fehlender Lesezugriff auf das Repository. |
| **Needs attention / Aufmerksamkeit nötig** | Mindestens ein belegter Handlungsbedarf: offene Dependabot-PRs, Pflegekandidaten, PR-Konflikte, fehlgeschlagene PR-Checks, zuverlässig erkannte neue Commits seit der Vergleichsbasis oder veraltete Website-Version. |
| **Unknown / Unbekannt** | Kein belegter höher priorisierter Zustand, aber die für Health nötigen Fakten fehlen, sind veraltet oder unvollständig. Dazu zählen Netzwerk-/API-Fehler, unbekannte Mergeability soweit relevant und noch nicht erfolgreich bewertbare Default-Branch-CI. |
| **Healthy / In Ordnung** | Die relevanten Fakten sind erfolgreich und aktuell geprüft; es gibt keinen erkannten Blocker oder Handlungsbedarf. |

Für mehrere Befunde gilt: **Blocked vor Needs attention vor Unknown vor Healthy**. Zeige zusätzlich die relevanten Gründe und Datenlücken. Ein bekannter Pflegebedarf darf trotz fehlender Release-Daten sichtbar bleiben; unvollständige Daten dürfen nie ein „alles in Ordnung“ erzeugen.

Weitere Abgrenzungen:

- Ein einzelner roter PR blockiert seine Merge-Aktion. Er macht nicht automatisch das gesamte Repository oder andere unabhängige Aktionen unbenutzbar.
- Ein fehlender lokaler Clone allein ist ein Hinweis. GitHub-basierte Übersicht und Pflege können ohne Clone funktionieren. Eine konfigurierte, aber ungültige Clone-Zuordnung betrifft insbesondere lokale Aktionen.
- Fehlende Schreibrechte oder Archivierung schließen das Lesen nicht aus. „Nur lesbar“ beziehungsweise „Archiviert“ anzeigen und Pflegeaktionen sperren.
- Fehlendes `gh`-Login ist nicht automatisch ein Fehler beim Lesen öffentlicher Repositories. Der bestehende öffentliche Lesezugriff darf genutzt werden; Mutation und Claude-Pflege folgen ihren bisherigen Authentifizierungsanforderungen.
- Ein 404 allein beweist nicht, dass ein Repository gelöscht wurde; es kann auch privat und nicht zugänglich sein. Ebenso einen 403 nicht ohne Prüfung pauschal als Rate Limit ausgeben.
- Normale offene PRs allein müssen kein Pflegeproblem sein. Unspezifische Warnungen wie „lange offen“ zunächst weglassen, solange eine klare fachliche Regel fehlt.
- Fehlende optionale Release- oder Cleanup-Daten allein müssen Healthy nicht verhindern. Verbindliche Mindestdaten sind Repository-/Zugriffsstatus, vollständige offene PR-Daten für die gewählten Health-Regeln und auswertbare Default-Branch-CI. Fehlende Checks oder laufende CI ergeben ohne stärkeren Befund zunächst Unknown.

„Pflege nötig“ aus den Gründen ableiten und bei unzureichender Datenlage als unbekannt darstellen. Keine zweite, widersprüchliche Ja/Nein-Logik einführen.

## 7. Rein lesender Health-Check und Datenabruf

Die Übersicht und „Status aktualisieren“ benötigen weder Claude noch vollständige SFTP-Einstellungen. Der Health-Check liest GitHub-Daten und vorhandene lokale Informationen. Er startet keine Pflegeplanung und kostet keine Claude-Nutzung.

**Verboten im Health-Check und in der Pflege-Analyse:** Merge, Kommentar, Rebase-Anforderung, Close, Tag, Delete, Clone, Fetch/Prune, Checkout, Änderungen am Arbeitsbaum oder an Projektdateien. App-interner Status darf aktualisiert werden.

Für Cleanup zunächst vorhandene lokale Referenzen lesen und mit GitHub-Fakten vergleichen. Reichen die Informationen nicht aus, Kandidaten als nicht sicher bestimmbar behandeln. Ein nötiges `fetch --prune` als lokalen Synchronisationsschritt ausdrücklich in die Vorschau aufnehmen und erst beim bestätigten Ausführen erledigen. Danach nur bereits genehmigte Kandidaten erneut validieren; neu entdeckte Kandidaten benötigen eine neue Vorschau.

Gemeinsame **lesende** Hilfsfunktionen aus der bestehenden Pflege extrahieren. Den teuren bisherigen Scan einschließlich aller PR-Reviews und Branch-Vergleiche nicht beim Öffnen jedes Screens ausführen.

- Repository-Liste sofort aus der Konfiguration aufbauen und Status schrittweise laden.
- Einmaliges Laden beim ersten Öffnen und eine manuelle Aktualisierung genügen. Kein dauerndes Hintergrund-Polling einführen.
- API-Abfragen begrenzt parallelisieren, gleiche Daten wiederverwenden, Detaildaten bei Bedarf nachladen.
- Pagination berücksichtigen. Bei gesetzten Obergrenzen abgeschnittene Listen markieren, Zähler gegebenenfalls als „mindestens N“ anzeigen und daraus keine vollständige Sicherheitsentscheidung ableiten.
- Fehler pro Repository bzw. Datenbereich isolieren; ein unerreichbares Repository darf die anderen nicht ausblenden.
- Zeitlimits, Ladezustände, Wiederholen und Abbruch berücksichtigen. Lange Arbeit nicht unter dem globalen UI-/Model-Lock ausführen.
- Ein explizites, zentral definiertes Aktualitätsfenster verwenden, für den MVP beispielsweise fünf Minuten. Das Fenster betrifft die Anzeige; es ersetzt niemals die Revalidation vor Änderungen.
- Alte Antworten nach Projektwechsel oder einem neueren Request verwerfen. Health-Refresh darf einen zur Bestätigung stehenden Plan weder ersetzen noch erweitern.

Branch-Cleanup-Hinweise nur bei belastbaren vorhandenen Daten anzeigen. Aufwendige vollständige Cleanup-Analysen dürfen dem ausdrücklich gestarteten Pflege-Check vorbehalten bleiben.

## 8. Release, Tag und „Commits seit …“

Die vorhandenen Begriffe haben verschiedene Bedeutungen:

- **Letztes veröffentlichtes Release:** das vorhandene GitHub-Release-Verfahren ohne Drafts und Prereleases.
- **Letzter passender stabiler Tag:** höchste passende `x.y.z`-Version für den konfigurierten `tagPrefix`, wie in der bestehenden Pflege. Ein Tag muss kein veröffentlichtes Release besitzen.
- **Dokumentierte Website-Version:** Wert des jeweiligen Produkts in `site.json`; das ist kein Beleg für den aktuell live ausgelieferten Stand.

Diese Informationen getrennt halten. In der Übersicht bevorzugt das veröffentlichte Release zeigen. Wenn keines vorhanden ist, einen ermittelten passenden Tag als **Tag** kennzeichnen. Keine lexikografische Versionssortierung und kein geratenes `tagPrefix` verwenden.

Für „Commits seit Release“ den Release-Tag zum Commit auflösen und gegen einen festgehaltenen Default-Branch-HEAD vergleichen. Nur bei belegter Vorfahrbeziehung eine Zahl anzeigen. Bei identischem Commit ist `0` korrekt; bei fehlender Basis, Fehlern, fehlenden Feldern oder divergierter Historie „nicht bestimmbar“ mit Grund anzeigen.

Wird ohne veröffentlichtes Release ein passender Tag als Basis verwendet, lautet die Anzeige **„Commits seit Tag …“**. Bei mehreren Release-Linien die Produkt-/Tag-Zuordnung sichtbar halten; mehrdeutige Fälle nicht automatisch entscheiden.

Der Website-Versionsvergleich verwendet wie bisher veröffentlichte Releases und die passende Produktkonfiguration. Ein neuer Tag allein bedeutet noch nicht „Website veraltet“.

**Bereits vorhanden und zu erhalten:** Die Pflege plant den nächsten Patch-Tag auf Grundlage passender stabiler Tags; auch vorhandene unveröffentlichte Änderungen ohne neue Merges können dafür relevant sein. Das erste Release ohne passende Ausgangsversion bleibt manuell. Keine zusätzliche KI-basierte SemVer-Empfehlung einführen.

## 9. Pflegeablauf und Sicherheitsbedingungen

Der Ablauf bleibt:

```text
Prüfen → Vorschau → ausdrücklich Ausführen → Ergebnis nachprüfen
```

Health-Prüfung und kostenpflichtige Claude-Pflege deutlich unterscheiden. Modellwahl, Budgetverhalten, Logs und vorhandene Review-Kriterien erhalten. Claude bleibt Berater; die Rust-Logik entscheidet über zulässige Aktionen. Die Review-Werkzeuge dürfen durch diesen Umbau nicht erweitert werden.

### Vorschau und Ausführung binden

- Pläne im Backend halten. Das Frontend bestätigt nur einen vorhandenen Plan anhand einer ID oder gleichwertigen Revision; es liefert keine frei ausführbaren Aktionsdaten.
- Plan an Projektkontext, explizite Repository-Ziele, Pflegeoptionen, relevante Konfiguration, Prüfzeitpunkt und beobachtete Commit-/Branch-Identitäten binden.
- Bei neuem Pflege-Check, Verwerfen oder relevanten Kontextänderungen alte Pläne ungültig machen. Eine Health-Aktualisierung invalidiert einen Plan nur bei relevanter erkannter Änderung, niemals durch stilles Ersetzen.
- Bestätigen und als laufend markieren atomar durchführen. Doppelklick oder wiederholter Command darf keinen zweiten Lauf starten.
- Bestehende Job-Sperre auch im Backend erhalten. Website-Schreibabläufe und Pflege nicht gleichzeitig ausführen.

### Unmittelbar vor jeder Mutation erneut prüfen

| Aktion | Mindestbedingungen |
| --- | --- |
| **Merge** | Geplantes Repository, PR weiterhin offen und kein Draft; gleicher Head-Commit, unverändertes Ziel-Repository und erlaubter Zielbranch; aktuelle Mergeability, positive CI und GitHub-Schutzregeln; gültiges, zum geprüften Stand gehörendes Review mit erlaubtem `green` oder `yellow`. Den erwarteten Head-SHA beim Merge übergeben. |
| **Close** | PR weiterhin offen, unveränderter Head und Zielkontext; gültige Begründung für Überflüssigkeit. Referenzierte Ersatz-PRs oder bereits enthaltene Änderungen müssen weiterhin zur Begründung passen. Kommentar und Schließen getrennt protokollieren, wenn nur ein Teil gelingt. |
| **Dependabot-Rebase** | PR weiterhin offen, tatsächlicher Dependabot-PR, unveränderter geprüfter Head und Zielkontext, weiterhin relevanter Konflikt oder Rückstand. Bei geänderten Voraussetzungen überspringen. Auch `@dependabot rebase` ist eine Mutation. |
| **Tag** | Geplante Version und Release-Linie weiterhin gültig, Tag noch nicht vorhanden, Änderungen seit Basis zuverlässig belegt; konkreter Zielcommit entspricht dem revalidierten geplanten Ergebnis. CI genau dieses Commits positiv abgeschlossen. Unmittelbar vor Erstellung Branch-/Tag-Zustand erneut prüfen. |
| **Remote-Branch löschen** | Branch weiterhin auf erwartetem SHA, nicht aktueller Default Branch, nicht geschützt, kein offener PR benötigt ihn; Merge-/Enthaltensein-Nachweis weiterhin gültig. Löschung nach geplantem Merge nur bei dessen Erfolg. |
| **Lokalen Branch löschen** | Clone gehört noch zum richtigen Repository; Branch weiterhin auf erwartetem SHA, nicht Default Branch und in keinem Worktree ausgecheckt; gültiger Merge-Nachweis. Kein pauschales Erzwingen aus einem alten Plan. |

Bei fehlenden oder widersprüchlichen Sicherheitsdaten die betroffene Aktion überspringen und den Grund anzeigen. GitHub-Schutzregeln nicht umgehen. Wo die API keine atomare Vorbedingung ermöglicht, verbleibende Zeitfenster offen benennen und keine absolute Race-Freiheit behaupten.

Zusätzliche verbindliche Regeln:

- `red`, fehlende, ungültige, doppelte oder nicht zum PR passende Bewertungen erlauben keinen Merge. `yellow` benötigt weiterhin die begründete Unbedenklichkeit nach den bestehenden Review-Regeln. Schema und semantische Zuordnung im Backend validieren.
- Bestehende Risikoklassen, Dateiprüfungen, SHA-Pin-Prüfungen und Regeln zu zusammengehörigen Abhängigkeiten erhalten. Abhängige Aktionen dürfen bei einem fehlgeschlagenen Vorgänger nicht blind weiterlaufen; keine atomare Mehr-PR-Transaktion versprechen.
- Eigene, erfolgreich ausgeführte Merges dürfen erwartete Folgeschritte ermöglichen. Unabhängige zwischenzeitliche Änderungen dürfen weder eine alte Bewertung wieder gültig machen noch ungeprüft in einen Tag einfließen. Falls die geplante Grundlage nicht mehr belegbar ist: neue Prüfung und Vorschau.
- Ein durch einen eigenen Merge entstandener Dependabot-Konflikt darf wie bisher zur Rebase-Anforderung führen, wenn diese bedingte Alternative in der Vorschau erklärt wurde. Sonst ist eine neue Vorschau nötig.
- Automatische Tags nur bei positiv bestätigter CI: keine Tags bei rot, pending, noChecks, unknown, Timeout oder Abbruch. Die vorhandene maximale CI-Wartezeit von 30 Minuten kann bleiben; kein CI-Erfolg aus bloßem Zeitablauf.
- Ein Tag darf nie still auf eine andere Version ausweichen, bestehende Tags verschieben oder ersetzen. Bei teilweise fehlgeschlagenen geplanten Merges nicht automatisch den verbleibenden Zustand als vollständiges Release taggen.
- Branch-Cleanup bleibt optional. Geschützte, ausgecheckte oder noch benötigte Branches erhalten. GitHubs automatische Branch-Löschung nach Merge berücksichtigen und bereits erfolgte Schritte nicht erneut auslösen.

Zukünftige Codeänderungen an verwalteten Produkt-Repositories sollen über **Branch → Änderung → Test → PR** laufen. Das ist keine Anweisung, den vorhandenen Website-Update-/Commit-/Push-Workflow in diesem Auftrag umzubauen.

## 10. Den Plan verständlich darstellen

Gruppiere pro Repository: **Merge, Close, Rebase, Tag, Cleanup, Bleibt offen**. Ein gesetzter Tag startet gegebenenfalls einen vorhandenen Release-Workflow; ihn nicht bereits als erfolgreich veröffentlichtes Release darstellen.

Jede Aktion zeigt Ziel, Grund, Voraussetzungen und Herkunft:

```text
OpenReleasePilot

Merge
→ #84 bump foo 1.2.3 → 1.2.4
  Claude: Risiko grün, Begründung …
  App-Prüfung: Zielbranch und CI passend; vor Ausführung erneute Prüfung

Close
→ #81 bump bar 3.1 → 3.2
  Claude: durch #87 überholt; wird vor Schließen erneut geprüft

Tag
→ 0.2.3, nur nach erfolgreicher Ausführung und CI des Zielcommits

Cleanup
→ 3 konkret benannte Branches; lokal und auf GitHub getrennt
```

Geplante Aktionen nicht mit Erfolgshäkchen als erledigt darstellen. Bedingte Aktionen, lokale Synchronisation und mögliche Rebase-Alternativen sichtbar machen. Keine neu entdeckten Mutationen ohne neue Vorschau hinzufügen.

Nach dem Lauf pro Aktion **erfolgreich, übersprungen, fehlgeschlagen oder abgebrochen** zeigen. Teilerfolge bleiben erhalten; kein globaler Erfolg bei einzelnen Fehlern. Abbruch verhindert weitere Aktionen, rollt bereits erfolgte Änderungen aber nicht zurück. Bei unklarem Ergebnis einer Schreibanfrage erst den Zustand lesen, bevor erneut geschrieben wird.

Betroffene Repository-Fakten anschließend lesend aktualisieren. „Verify“ bedeutet hier die Nachprüfung der Repository-Aktionen; die bestehende Website-Verifikation bleibt ein eigener Schritt.

## 11. Umsetzungsschritte

1. Architektur und bestehende Tests prüfen; die benannten Abweichungen am aktuellen Stand bestätigen.
2. Identität, Produktzuordnungen und expliziten Pflegeumfang von der Dialog-/Produktauswahl entkoppeln.
3. Lesende Faktenbeschaffung und deterministische Health-Auswertung im Core ergänzen.
4. Tauri-State und Commands für Übersicht, Detaildaten und Aktualisierung ergänzen; Datenqualität und Kontextwechsel berücksichtigen.
5. Hauptnavigation und zunächst lesende Übersicht/Detailansicht implementieren.
6. Bestehenden Pflegeplan und dessen Ausführung integrieren; Sicherheitslücken gezielt schließen.
7. Aussagekräftige Tests ergänzen und die vorhandenen Build-/Testprüfungen ausführen.
8. Beide READMEs aktualisieren und die implementierten Grenzen dokumentieren.

Kleine, überprüfbare Änderungen bevorzugen. Für neue abstrahierte GitHub-/Git-Zugriffe nur so viel Testbarkeit ergänzen, wie für echte Sicherheitsfälle nötig ist. Keine unnötige allgemeine Service-Architektur aufbauen.

## 12. Tests und Abnahme

Automatisierte Tests verwenden Fixtures oder simulierte GitHub-/Git-Antworten. Sie benötigen keine echten Tokens, keine Claude-Kosten und keine Mutationen an echten Repositories. Bestehende ausdrücklich ignorierte Netzwerktests nicht unbemerkt Teil der Standardsuite machen.

Mindestens folgende Fälle absichern:

| Bereich | Nachweis |
| --- | --- |
| Identität und Scope | Dasselbe Repository aus mehreren Produkten erscheint einmal; Beziehungen bleiben erhalten. Abgewählte Produkte bleiben sichtbar. Einzelrepo-Pflege betrifft kein anderes Repository. Konfligierende Konfiguration löst keine geratenen Tag-/Clone-Aktionen aus. |
| Health | Default-CI fehlgeschlagen → Blocked; belegter Pflegebedarf → Needs attention; vollständiger aktueller sauberer Zustand → Healthy; fehlende/abgeschnittene/veraltete Daten → kein falsches Healthy. Priorität mehrerer Gründe testen. |
| Capability und CI | Öffentliche Daten ohne Login lesbar; fehlende Schreibrechte sperren Pflege, nicht Übersicht. Rot, pending, noChecks und unknown bleiben unterscheidbar; PR-Fehler sind kein pauschaler Repository-Blocker. |
| Read-only | Health und Pflege-Analyse führen keine Schreibaufrufe und kein Fetch/Prune aus; Health startet kein Claude. |
| Release-Basis | Release und Tag unterscheiden; passende Präfixe und numerische Reihenfolge; fehlende Basis, divergierte Historie und Fehler ergeben kein erfundenes `0`. Website-Vergleich verwendet veröffentlichtes Release. |
| Plan-Lebenszyklus | Projekt-/Optionswechsel, neue Vorschau, verspätete Antwort, Doppelklick und parallel gestarteter Job führen weder zu falschem Scope noch zu doppelter Ausführung. |
| Merge/Review | Kein Merge ohne gültige passende Bewertung, bei red oder unbekanntem risk; erlaubtes yellow bleibt möglich. Geänderter Head/Zielbranch, rote oder fehlende CI verhindern den Merge. |
| Revalidation | Geänderter Rebase-PR, entfallene Close-Begründung, neu geschützter/benötigter Branch oder anderer Clone verhindern die betroffene Mutation. Nicht nur die Planerstellung, sondern tatsächliche Schreibaufrufe prüfen. |
| Tagging | Kein Tag ohne positive CI für exakt den Zielcommit; kein Tag bei Abbruch, Timeout, unerwartetem Branch-Fortschritt oder unvollständiger Ausführung vorausgesetzter Merges. Existierende Tags bleiben unverändert. |
| Cleanup und Teilerfolg | Optionales Cleanup respektiert SHA, offene PRs und alle Worktrees; fehlgeschlagener Merge verhindert seine abhängige Löschung. Fehler/Abbruch erhalten Teilergebnisse; Wiederholen erzeugt keine doppelten Aktionen. |

Zusätzlich manuell prüfen: Navigation, Leer-/Lade-/Fehlerzustände, mehrere Produkte pro Repository, lange Namen, Tastaturbedienung, laufender Job während Ansichtswechsel und eine veraltete Vorschau.

Im Verzeichnis `desktop` mindestens `cargo test -p openreleasepilot-core` und `cargo check --workspace` sowie vorhandene relevante CI-Prüfungen ausführen. Falls Tauri-/Orchestrierungslogik neue Tests erhält, diese ebenfalls ausführen. Nicht ausgeführte Prüfungen mit Grund nennen.

Bestehende Funktionen müssen erhalten bleiben: Precheck, Update, Upload, Verify, New product, Produkt- und Repository-Auswahl, Clone-Erkennung, GitHub-CLI-Integration, Claude-Ausführung, Pflegeplanung, sicheres Tagging und optionales Branch-Cleanup. Für reale Merge-/Tag-/Delete-Aktionen keinen automatischen Live-Smoke-Test verwenden.

## 13. Dokumentation und Definition of Done

`README.md` **und** `README.de.md` aktualisieren:

- Produktbeschreibung um Repository-Pflege und Release Operations erweitern, ohne spätere Funktionen als vorhanden darzustellen.
- Neuen Hauptbereich, Repository-Umfang und Verhältnis zur Website-Produktauswahl erklären.
- Kostenfreien lesenden Statuscheck von der Claude-Pflege unterscheiden.
- Vorschau, Sicherheitsbedingungen, Teilerfolge und die gezielte Verschärfung beim Tagging ohne CI beschreiben.
- Weiterhin erklären, was Website-Update, Upload und Verify konkret tun.

Die Umsetzung ist fertig, wenn:

- [ ] Repositories ein sichtbarer Hauptbereich ist und alle konfigurierten Produkt-Repositories dedupliziert enthält.
- [ ] Jede Zeile einen verständlichen Zustand samt Grund, Prüfzeitpunkt und gegebenenfalls Datenlücke zeigt.
- [ ] Details für Status, PRs, CI, Release/Tag, Clone und Website-Beziehung erreichbar sind.
- [ ] Einzelrepo- und bisherige Mehrfachpflege dieselbe Kernlogik verwenden.
- [ ] Der Health-Check rein lesend und ohne Claude nutzbar ist.
- [ ] Jede Mutation aus einer bestätigten Vorschau stammt und unmittelbar vorher ihre Sicherheitsbedingungen erneut prüft.
- [ ] Die ausdrücklich benannten Härtungen umgesetzt und durch aussagekräftige Tests belegt sind.
- [ ] Navigation, Abbruch, Fehler und Teilerfolge keinen Plan verlieren oder unbemerkt wieder ausführen.
- [ ] Bestehende Konfigurationen und der Website-Workflow weiter funktionieren.
- [ ] Relevante Tests und Buildprüfungen erfolgreich sind; beide READMEs den tatsächlichen Stand beschreiben.

Zum Abschluss knapp berichten: umgesetzte Funktionen, wesentliche Architekturänderungen, bewusste Verhaltensänderungen, ausgeführte Prüfungen und verbleibende Grenzen. Keine späteren Care-/Release-/Publish-Funktionen als erledigt ausgeben.
