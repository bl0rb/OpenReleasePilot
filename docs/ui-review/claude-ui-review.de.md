# OpenReleasePilot: UI/UX-Review für Claude

Stand: 14. Oktober 2026 · Screenshot-Version: 0.3.4

## Zweck und Arbeitsgrundlage

Dieses Dokument enthält den Review und konkrete Empfehlungen zur Überarbeitung. Es wurden noch keine Änderungen an der Anwendung vorgenommen.

Bei einer Umsetzung: Verifiziere die Befunde erneut am aktuellen Code und verbessere die bestehende Oberfläche gezielt. Behalte Rust, Tauri und das vorhandene JavaScript/CSS bei. Kein neues Frontend-Framework, keine vollständige Neuschreibung und keine zusätzlichen Produktbereiche.

Grundlagen:

- `docs/ui-review/screenshots/`: Bilder aus der installierten App, helles Erscheinungsbild, Fenster laut Aufnahmeprotokoll 1280 × 1040 pt.
- `docs/ui-review/README.md`: Zuordnung der Screenshots.
- `desktop/ui/app.js`, `desktop/ui/style.css`, `desktop/ui/index.html`: tatsächliche Oberfläche.
- Relevante Rust-Stellen wurden bei Verhaltensbefunden zusätzlich gelesen.
- `docs/claude-repository-first-follow-up.de.md`: bewusste Produktentscheidungen; insbesondere Repository-Einstieg, optionale Website und persistente Pflegeaufträge.

Codeangaben unten sind repositoryrelativ; die Zeilennummern beziehen sich auf den beim Review vorliegenden Arbeitsstand und können sich verschieben. Suche zusätzlich nach den genannten Funktionen.

**Vor Änderungen den Arbeitsbaum prüfen.** Beim Review gab es bereits Änderungen in `desktop/core/src/github.rs` und `desktop/src-tauri/src/jobrun.rs` sowie unversionierte Dokumentation und Screenshots. Vorhandene Arbeit erhalten und nicht zurücksetzen.

## Gesamteindruck: 7/10

Die App hat eine passende fachliche Grundlage: Repositories stehen im Mittelpunkt, offene Entscheidungen werden hervorgehoben, Ergebnisse sind vom Log getrennt und Veröffentlichungen besitzen einen nachvollziehbaren Freigabeablauf.

Das größte UX-Potenzial liegt in der Informationshierarchie. Globale Aktionen, technische Fakten, Einrichtung und operative Entscheidungen beanspruchen häufig ähnlich viel Aufmerksamkeit. Mehrere lange Erklärtexte und verschachtelte Bereiche erschweren das schnelle Erfassen des nächsten Schritts. Hinzu kommen konkrete Abweichungen zwischen Button-Beschriftung und tatsächlicher Wirkung.

## Das beibehalten

- **Repositories als Startbereich.** Die Entscheidungskarten machen die Startseite bereits zu einer Arbeitshilfe. Kein zusätzliches Statistik-Dashboard nötig.
- **Website als optionalen Hauptbereich.** Der direkte Website-Ablauf ist ausdrücklich vorgesehen und bleibt erhalten.
- **Repository-Pflegeauswahl und Website-Produktauswahl bleiben getrennt.** Keine gegenseitigen stillen Änderungen.
- **Prüfen, Vorschau, Freigabe und Ergebnis bleiben unterscheidbar.** Eine kompaktere UI darf die Wirkung von Aktionen nicht verstecken.
- **Ergebnis und Log bleiben getrennt.** Die Tabs besitzen bereits Tastatursteuerung; nach Auftragsende wird automatisch auf das Ergebnis gewechselt.
- **Release und Tag bleiben unterschiedliche Aussagen.** Ein erstellter Inhalts-PR gilt weiterhin nicht automatisch als veröffentlicht.
- **Offene Aufträge sperren betroffene Änderungen weiterhin.** Sperren verständlicher erklären, nicht entfernen.
- **Leere Zustände und Vorauswahlen erhalten.** Repository-Onboarding, Suche im Hinzufügen-Dialog, lokale Klonerkennung und README-Vorauswahl sind bereits vorhanden.
- **Bestehende Sicherheits- und Berechtigungsprüfungen bleiben erhalten.** Kein allgemeiner zusätzlicher Admin-Bereich: lokale App-Einrichtung und GitHub-Rechte sind die vorhandenen Konzepte.

## Priorisierung

**P0:** Im geprüften Material kein belastbarer P0-Befund.

**P1:** UX-01 bis UX-09. Zuerst unerwartete Nebenwirkungen und falsche Zustandsmeldungen korrigieren, danach die zentralen Arbeitsabläufe vereinfachen.

**P2:** UX-10 bis UX-13. Komfort, Skalierbarkeit und visuelle Nacharbeit nach den P1-Punkten.

## P1: konkrete Befunde und Änderungen

### UX-01 — Verbindungstest speichert die gesamte Einrichtung

**Beobachtung:** In `12a/12b` stehen „Verbindung testen“, „Abbrechen“ und „Sichern“ nebeneinander. Der Test wirkt unverbindlich, speichert aber vor dem Verbindungsaufbau sämtliche übergebenen Einstellungen. Auch nach einem fehlgeschlagenen Test kann „Abbrechen“ diese Speicherung nicht rückgängig machen.

**Code:** `app.js:2082–2097`, `settingsDialog`; `desktop/src-tauri/src/main.rs:578–580`, `test_connection`, ruft `apply_settings` auf. Dort werden die Einstellungen zugewiesen und in Zeile 560 gespeichert.

**Änderung:** Verbindung mit temporären Eingaben testen. Erst „Sichern“ persistiert die Einrichtung. Testergebnis direkt beim SFTP-Abschnitt beziehungsweise bei der Testaktion anzeigen. Falls Persistenz technisch erforderlich bleiben sollte, muss die Aktion ausdrücklich „Speichern und Verbindung testen“ heißen; die temporäre Variante ist vorzuziehen.

**Abnahme:** Projektordner, Modell oder Budget ändern → Verbindung testen → Abbrechen: Die gespeicherten Werte bleiben unverändert. Ein fehlerhafter Verbindungstest speichert ebenfalls nichts.

### UX-02 — Ungültige Kostengrenze entfernt still die appweite Grenze

**Beobachtung:** Das Feld erklärt „leer = keine“. Auch `-5`, `0` oder nicht lesbare Eingaben werden jedoch als `null` gespeichert. Dadurch entfällt die appweite Grenze ohne Fehlermeldung. Je nach Auftrag kann noch ein Website-Standard greifen; der Befund bedeutet nicht, dass jeder betroffene Lauf garantiert unbegrenzt ist.

**Code:** `app.js:1997–2000`, `settingsDialog.input`; `main.rs:547`; Budget-Fallback in `desktop/src-tauri/src/jobrun.rs:55`.

**Änderung:** Nur ein ausdrücklich leeres Feld entfernt die Grenze. Andere Eingaben müssen eine endliche positive Zahl ergeben. Komma und Punkt als Dezimaltrennzeichen unterstützen. Fehler direkt am Feld anzeigen; Speicherung ungültiger Werte auch im Backend ablehnen.

**Abnahme:** Leer, `1,50` und `1.50` funktionieren wie beschrieben. `-5`, `0` und Text erzeugen einen verständlichen Feldfehler und verändern die gespeicherte Grenze nicht.

### UX-03 — „Alle“ wählt auch ausgeblendete Repositories

**Beobachtung:** Im Hinzufügen-Dialog (`03`) stehen Suche und Sammelauswahl zusammen. Die Liste wird gefiltert, „Alle“ arbeitet aber auf sämtlichen noch nicht verwalteten Repositories des Ordners. Nach einer Suche können dadurch unsichtbare Repositories mit ausgewählt werden.

**Code:** `app.js:1777` filtert die Liste; `1806` und `1813–1816` verwenden für die Sammelauswahl die ungefilterte Menge in `addReposDialog`.

**Änderung:** „Alle Treffer auswählen“ und „Treffer abwählen“ beziehen sich auf die sichtbaren, auswählbaren Ergebnisse. Eine bereits bestehende Auswahl außerhalb des Filters bleibt bestehen und wird als „X ausgewählt, davon Y ausgeblendet“ kenntlich gemacht.

**Abnahme:** Nach Filtern auf ein Repository verändert die Sammelauswahl ausschließlich dieses sichtbare Repository. Die Übernahmezahl entspricht der tatsächlichen Gesamtauswahl.

### UX-04 — „Aus der Liste entfernen“ verschleiert dauerhaftes Löschen

**Beobachtung:** In `09a` entfernt die neutral beschriftete Aktion ohne Bestätigung nicht nur die Zeile, sondern auch den gespeicherten Auftrag, sein Log und seinen zugehörigen Arbeitsordner.

**Code:** `app.js:1350`, `renderJob`; `main.rs:1316–1325`, `delete_job`; `desktop/core/src/jobs.rs:923–926`, `remove`.

**Änderung:** Als sekundäre Aktion „Auftragsverlauf löschen …“ anbieten. Vor dem Löschen konkret benennen, dass gespeicherte Auftragsdaten, Log und temporäre Arbeitsdaten entfernt werden. Bereits ausgeführte externe Änderungen werden dadurch nicht rückgängig gemacht. Keine neue Archivierungsfunktion erfinden.

**Abnahme:** Öffnen und Abbrechen der Bestätigung verändert keine Daten. Erst die ausdrückliche Löschaktion entfernt den abgeschlossenen Auftrag.

### UX-05 — Bereits erreichte Ziele erzeugen falschen Handlungsbedarf

**Beobachtung:** In `09a/09b` sind zwei Branches „war schon gelöscht“. Trotzdem wird der Auftrag als „Teilweise erfolgreich“ angezeigt und ein neuer Auftrag für das Übrige empfohlen. Die App meldet diese Löschziele selbst als bereits erreicht; der tatsächliche Remote-Zustand wurde im Review nicht geprüft.

**Code:** `desktop/core/src/repocare.rs:894` liefert `Done::Skipped`; `jobrun.rs:413` übernimmt das Ergebnis, `439–466` leitet daraus einen Teilerfolg ab. `jobs.rs:550–560` erzeugt den globalen Warnstatus und Wiederholungsrat.

**Änderung:** Nachweislich bereits erreichte Ziele semantisch von tatsächlich übersprungenen oder blockierten Aktionen unterscheiden. Dafür keine pauschale Umdeutung aller `Skipped`-Ergebnisse verwenden. „Branch hat neue Commits“ und andere Sicherheitsstopps müssen weiterhin als unerledigt gelten. Vor der Änderung prüfen, ob die verwendete Remote-Abfrage Abwesenheit zuverlässig von Fehlern trennt.

**Ergebnisdarstellung:** „3 PRs gemergt · 1 Branch gelöscht · 2 bereits gelöscht“. Nur tatsächlich offene Arbeit begründet eine Wiederholung. Nach Ausführung „3 gemergt“ statt „3 mergen“ anzeigen; Planbedingungen nicht weiter als aktuelles Ergebnis präsentieren (`app.js:1503–1528`).

**Abnahme:** Bereits gelöschte Branches allein erzeugen keine unnötige Wiederholungsaufforderung. Echte Fehler, unklare Abfragen und Sicherheitsstopps bleiben sichtbar. Gespeicherte ältere Aufträge bleiben lesbar.

### UX-06 — Repository-Details mischen globale und lokale Bedienung

**Beobachtung:** In `02a` und `05a` stehen oberhalb der eigentlichen Repository-Details weiterhin die globale Entscheidungskarte und die Flottenaktionen. „Status aktualisieren“ erscheint zweimal mit unterschiedlichem Umfang. Beim Öffnen von Gartenamt dominiert zunächst der fremde nudgit-Auftrag. Pull Requests liegen unter einem langen Faktenblock und der noch ungeprüften Sicherheitssektion.

**Code:** `app.js:693–738`, `renderRepos`, rendert den globalen Kopf auch im Detail; `743–750`, `renderDecisions`, bleibt unbeschränkt sichtbar. `870–899`, `repoDetail`, hängt Status, Sicherheit, PRs, Inhalte und Verwaltung hintereinander.

**Änderung in der Detailansicht:**

1. Direkt mit „Alle Repositories → Repositoryname“ und den Aktionen für dieses Repository beginnen.
2. Globale Flottenaktionen auf der Übersicht belassen. Offene Entscheidungen anderer Repositories bleiben über die Hauptnavigation erreichbar.
3. Einen Auftrag für das aktuelle Repository als kompakten Hinweis mit „Auftrag ansehen“ anzeigen.
4. Konkrete Befunde und Pull Requests vor die ausführlichen technischen Fakten setzen. Relevante Sicherheitsbefunde weiterhin deutlich hervorheben.
5. CI, Release und Datenstand knapp zusammenfassen; volle Pfade, Commitkennungen und zusätzliche Fakten in einen aufklappbaren Abschnitt verschieben.
6. Verwaltung als eigenen aufklappbaren Abschnitt am Ende führen. „Speichern“ soll erst bei Änderungen relevant werden.

**Abnahme:** In Gartenamt ist sofort erkennbar, dass alle sichtbaren Hauptaktionen Gartenamt betreffen. Es gibt dort nur eine lokale Statusaktualisierung. Die globale Freigabe bleibt auffindbar, drängt sich aber nicht vor den fremden Repository-Kontext.

### UX-07 — Freigabe zeigt technische Details vor der Entscheidungsvorlage

**Beobachtung:** In `08a/08c` dominieren Commitkennung, Git-Statistik und Rohdiff. Die vorhandene Zusammenfassung ist eingeklappt. Die Veröffentlichung ist korrekt erklärt, aber die fachliche Änderung könnte wesentlich schneller erfassbar sein.

**Code:** `app.js:1616–1635`, `websiteBlock`: Fakten → Statistik → geschlossene Zusammenfassung → Diff → Freigabe. Der Diff kann in `style.css:457` bis zu `60vh` belegen.

**Änderung:** Vor dem Diff eine kompakte, sichtbare Entscheidungsvorlage aus vorhandenen Daten darstellen: betroffenes Produkt/Ziel, Versionsänderung, vorhandene Zusammenfassung und Wirkung der Freigabe. Beispiel: „nudgit auf der Website: 1.2.2 → 1.2.3“. Prüfergebnisse nur ergänzen, soweit sie als verlässliche Daten verfügbar sind; keine Erfolgszustände aus Freitext oder bloß vorhandenen Vorschlägen ableiten. Der aktuelle `WebsiteProposal` enthält keine strukturierten Prüfergebnisse (`jobs.rs:119–127`). Keine neuen KI-Aufrufe und keine erfundenen Erfolgsmeldungen.

Die bestehende Zusammenfassung sichtbar voranstellen; Git-Statistik nachrangig aufklappbar machen. Die vorhandene technische Änderungsvorschau und Hinweise auf ihre Grenzen erhalten: Website-Diffs werden oberhalb von 300 KiB gekürzt und verweisen auf `git show` (`jobrun.rs:897–904`). Freigabe, Verwerfen und Neu vorbereiten während der Prüfung gut erreichbar halten, etwa in einer klar abgegrenzten Aktionsleiste. Die vorhandene Bindung der Freigabe an den konkreten Commit beziehungsweise Plan unverändert erhalten.

**Abnahme:** Ohne Lesen des Rohdiffs ist klar, was betroffen ist und was ein Klick bewirkt. Die technische Änderungsvorschau bleibt erreichbar; Kürzungen und der Verweis auf die vollständige lokale Ansicht bleiben kenntlich. Die Erklärung zu Veröffentlichung und lokal verbleibendem Commit nach Verwerfen bleibt sichtbar.

### UX-08 — Dialogänderungen entfernen Fokus und Scrollcontainer

**Beobachtung:** Repository-Auswahl, Pflegeoptionen und Einrichtungsänderungen ersetzen komplette Listen oder den ganzen Dialoginhalt. Das fokussierte Element wird dabei entfernt. Ein langer Dialog erhält einen neuen Scrollcontainer, ohne Wiederherstellung der Position. Das genaue Verhalten im laufenden WebView wurde nicht getestet; die fehlende Erhaltung ist im Code belegt.

**Code:** `app.js:1666–1669`, `openDialog.render`; `1783`, Repository-Auswahl; `1935` und `1951`, Pflegeoptionen; `2062` und `2082–2092`, Einrichtung. Außerhalb der Dialoge existiert bereits `keepFocus` in Zeile 687.

**Änderung:** Möglichst nur abhängige Teile aktualisieren. Wo neu gerendert werden muss, stabile IDs sowie Fokus und Scrollposition erhalten. Rückmeldungen in Reichweite der auslösenden Aktion anzeigen und programmatisch ankündigen. Native Dialogfunktionen beibehalten.

**Abnahme:** Mehrere Repositories und Pflegeoptionen lassen sich nacheinander per Tastatur bedienen. Ein Wechsel der SFTP-Anmeldeart oder Verbindungstest springt nicht unerwartet an den Dialoganfang. Testresultate sind unmittelbar erkennbar.

### UX-09 — Fehler in langen Formularen sind nicht dem Feld zugeordnet

**Beobachtung:** Das Inhaltsziel-Formular (`06/06b`) ist lang. Pflichtverletzungen werden erst beim Speichern geprüft und als allgemeine Meldung am Ende ausgegeben. Nutzer müssen das fehlerhafte Feld selbst suchen.

**Code:** `app.js:1876–1914`, `targetDialog`; Pflichtregeln in `desktop/core/src/content.rs:113–127`.

**Änderung:** Pflichtfelder sichtbar kennzeichnen. Einfache Pflicht- und Formatfehler früh am Feld anzeigen, mit `aria-describedby` zuordnen und nach gescheitertem Speichern das erste fehlerhafte Feld fokussieren. Backend-Validierung bleibt maßgeblich. Gültige Eingaben bei einem Fehler erhalten.

**Abnahme:** Fehlender Name, fehlende Quelle, ungültiges Zielrepository oder fehlende erlaubte Pfade führen zu einer konkreten Meldung am betroffenen Feld. Ein generischer Fehler außerhalb eines Feldes bleibt als sichtbare Formularmeldung erreichbar.

## P2: gezielte Verbesserungen

### UX-10 — Repository-Übersicht für größere Bestände vorbereiten

Die Karten funktionieren mit sieben Repositories. Die Hauptübersicht rendert jedoch die vollständige Liste ohne Suche oder Filter (`app.js:693–738`). Die Suche im Hinzufügen-Dialog ist bereits vorhanden und ist hiervon nicht betroffen.

**Empfehlung:** Über den Karten eine Namenssuche und wenige Filter auf bestehende Zustände anbieten, beispielsweise „Alle“, „Aufmerksamkeit“, „Blockiert“ und „Unbekannt“. Repository-Zustand und Auftragszustand weiterhin getrennt benennen. Auswahl außerhalb eines Filters nicht still verlieren. Bei leerem Filterergebnis „Filter zurücksetzen“ anbieten.

Keine zusätzliche Dashboard-Seite und keine automatische Hintergrundabfrage einführen. Die dokumentierte Fünf-Minuten-Aktualitätsregel nicht nebenbei ändern. Veraltete Daten weiter sichtbar kennzeichnen, ohne den letzten bekannten Stand als aktuell auszugeben.

### UX-11 — Lange Formulare und technische Metadaten besser gruppieren

**Inhaltsziel (`06/06b`):** In „Inhalt“, „Quellen und Ziel“ sowie „Aktualisierung“ gruppieren. Quellen kompakt und bei vielen Einträgen durchsuchbar darstellen. Skill, Modell und Prüfbefehl unter „Erweiterte Optionen“ bündeln (`app.js:1877–1904`). Veröffentlichungs- und Mergeverhalten sowie relevante Kostenhinweise bleiben sichtbar.

**Einrichtung (`12a/12b`):** Die vorhandenen Gruppen als „Allgemein“, „Website und Veröffentlichung“ sowie „App“ besser gliedern. Aufklappbare Abschnitte reichen; kein Wizard nötig. Die Aktionsleiste bleibt sichtbar.

**Website (`10a/10b`):** Aufgabenname, Beschreibung und Auswahl priorisieren. Nicht editierbare Skill-/Effort-Metadaten in aufklappbare technische Details verlagern (`app.js:389–397`). Die Modellwahl bleibt erreichbar.

Wichtige Erklärungen nicht in reine Hover-Tooltips verschieben. Aufklappfunktionen müssen mit Tastatur bedienbar sein.

### UX-12 — Begriffe und Schrittzustände konsistent machen

- Hauptnavigation „Läufe“ zu **„Aufträge“** vereinheitlichen; die Detailseite und das Produktmodell verwenden bereits diesen Begriff. Einzelne Claude-Ausführungen dürfen weiterhin „Läufe“ heißen.
- „Einrichtung“ zu **„Einstellungen“** umbenennen, da dort auch dauerhafte Betriebsregeln geändert werden.
- Website-Schritte verständlicher benennen: **„Prüfen → Inhalte aktualisieren → Veröffentlichen → Live-Seite prüfen“**. Bestehenden Ablauf erhalten.
- „Alle Schritte“ zu **„Aktualisieren und veröffentlichen“** präzisieren. Die Aktion kann bis zur Live-Veröffentlichung führen.
- „Neues Produkt …“ zu **„Produkt zur Website hinzufügen …“**, finale Aktion passend zur vorhandenen Inhaltserzeugung als „Produktseite erstellen“ formulieren. Die bereits vorhandene Erklärung zu Build und Commit erhalten.
- Schrittchips in Auftragskarten zeigen aktuell den Zustand nur über Farbe und `title` (`app.js:763`, `1293`). Text ergänzen, etwa **„Website · Freigabe nötig“** oder **„Release · Erledigt“**. Die Gesamtzustände besitzen bereits Text und brauchen keine neue Statussystematik.

### UX-13 — Dark Mode und visuelle Hierarchie nachprüfen

Der Dark Mode existiert bereits. Weißer Text auf dem dunklen Theme-Akzent `#8b85ff` ergibt bei Primärbuttons rechnerisch ungefähr 3,04:1; im hellen Theme ungefähr 6,29:1 (`style.css:31`, `82`). Einen eigenen Farbwert für gefüllte Buttons verwenden oder eine passende dunkle Textfarbe wählen. Beide Themes anschließend in der App visuell prüfen.

Visuell die vorhandene ruhige Gestaltung erhalten: klare Überschriften, weniger gleichgewichtige Rahmen und kürzere dauerhaft sichtbare Erklärungstexte. Pro Ansicht eine dominante Hauptaktion. Sicherheitsfarben weiterhin für Zustände verwenden und Text ergänzen. Mehr Schatten oder Animationen lösen die identifizierten Probleme nicht.

## Empfohlene Navigation

Die drei Hauptbereiche bleiben erhalten. Die eingerückten Detailpunkte sind vorhandene Inhalte, die als Abschnitte oder Sprungmarken organisiert werden können; dafür sind keine neuen Hauptseiten notwendig.

```text
ARBEIT
  Repositories
    Übersicht mit offenen Entscheidungen
    Repository-Detail
      Befunde und Pull Requests
      Zugeordnete Inhalte
      Sicherheit
      Technische Details
      Verwaltung
  Aufträge
    Offene Aufträge
    Bisherige Aufträge
    Auftrag-Detail
      Ergebnis und Freigabe
      Log
  Website (wenn eingerichtet)
    Ablauf
    Produkte
    Update-Aufgaben

EINSTELLUNGEN
  Allgemein
  Website und Veröffentlichung
  App
```

Repositorybezogene Verwaltung bleibt am Repository. Globale Zugangsdaten und Ausführungsregeln bleiben in Einstellungen. Sicherheit bleibt im Repository-Kontext. Die bestehenden Hinweise zu fehlenden GitHub-Adminrechten erhalten; kein neues Rollenmodell einführen.

## Umsetzung und Prüfung

1. Aktuellen Stand und vorhandene Änderungen prüfen. Die Befunde anhand der Funktionen erneut verifizieren.
2. UX-01 bis UX-05 als klar abgegrenzte Verhaltenskorrekturen bearbeiten. Relevante Regressionstests ergänzen, besonders für Speicherung, Budgetvalidierung, gefilterte Sammelauswahl und Ergebnisaggregation.
3. Repository-Details, Freigabeansicht und Dialogverhalten überarbeiten. Bestehende Backend-Kommandos und Freigaberegeln wiederverwenden.
4. P2-Verbesserungen auf die tatsächlich vorhandenen Funktionen begrenzen. Bestehende deutsche und englische Dokumentation bei geänderten Bedienabläufen konsistent halten.
5. Änderungen visuell und per Tastatur prüfen. Relevante Rust-Tests und `cargo check --workspace` im Verzeichnis `desktop` sowie eine JavaScript-Syntaxprüfung ausführen. Vorhandene weitere Checks beachten; keine neue Testinfrastruktur allein für kosmetische Anpassungen einführen.
6. Am Ende Änderungen, ausgeführte Prüfungen und verbleibende Einschränkungen nennen. Keine Releases, Uploads, Merges oder Änderungen an echten Repositories als UI-Test auslösen.

### Szenarien für die Abnahme

- Frischer Einstieg ohne Website: Repository aufnehmen und Pflegeauftrag vorbereiten.
- Mehrfachauswahl mit aktivem Filter und bereits ausgewählten ausgeblendeten Einträgen.
- Repository mit offenem Auftrag: Sperrgrund und Link zum Auftrag erkennbar.
- Repository mit mehreren PRs: Befunde und nächste Aktion schnell erreichbar.
- Auftrag mit Freigabe: fachliche Änderung, technische Vorschau und Veröffentlichungswirkung nachvollziehbar.
- Abgeschlossener Auftrag mit bereits gelöschten Branches sowie ein echter Teilerfolg als Gegenprobe.
- Ungültiges Inhaltsziel: Feldfehler, Fokus und erhaltene Eingaben.
- Verbindungstest mit geänderten, noch nicht gespeicherten Einstellungen; danach Abbrechen.
- Tastaturbedienung mehrerer aufeinanderfolgender Optionen in allen betroffenen Dialogen.
- Helles und dunkles Erscheinungsbild bei üblicher Fenstergröße und bei der erlaubten Mindestgröße 820 × 560 px.

### Grenzen dieses Reviews

Der Review basiert auf Screenshots und statischem Codeabgleich, nicht auf einem vollständigen Live-Test. Die Bilder zeigen ausschließlich das helle Erscheinungsbild. Dark Mode, tatsächlicher Fokusverlauf und Mindestfenstergröße benötigen eine Prüfung in der laufenden App. Ein mobiler Drawer ist für diese Desktop-App keine aus dem Material abgeleitete Anforderung.

`08a`, `08b` und `08c` sind bytegleich; ebenso `09a` und `09b`. Eine separat aufgeklappte Zusammenfassung ist dadurch trotz Dateiname nicht dokumentiert. Sicherheitsprüfungsergebnisse sowie mehrere Bestätigungs- und Ausführungszustände wurden laut Aufnahmeprotokoll nicht aufgenommen. Aus fehlenden Screenshots darf keine fehlende Funktion abgeleitet werden.
