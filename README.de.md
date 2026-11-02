[English](README.md) · **Deutsch**

<img src="desktop/assets/icon.svg" width="96" alt="OpenReleasePilot-Icon">

# OpenReleasePilot

Zwei Desktop-Apps in einem Repository, die sich die Rust-Bibliothek `desktop/core` teilen; Claude Code
erledigt die Arbeit, du triffst die Entscheidungen:

- **OpenReleasePilot** kümmert sich um die Repositories auf GitHub und GitLab, die du auswählst. Sie liest
  den Zustand, prüft und mergt Pull bzw. Merge Requests, räumt gemergte Branches auf, taggt Releases (Patch,
  Minor oder Major) und wartet auf das veröffentlichte Release und hält eine README, weitere Dokumentation
  oder ein Wiki aktuell. Alles, was auf GitHub, GitLab oder in einem Wiki etwas ändert, zeigt sie vorher
  als konkreten Vorschlag und führt es erst nach deiner Freigabe aus.
- **OpenSitePilot** aktualisiert und veröffentlicht eine statische Website, die deine Produkte beschreibt
  (siehe [OpenSitePilot](#opensitepilot-website)). Ohne Website brauchst du sie nicht.

OpenReleasePilot hat zwei Bereiche, umschaltbar in der Seitenleiste, und ihre Einstellungen unter
**Einstellungen** unten darin:

| Bereich | Wofür |
|---|---|
| **Repositories** (Startansicht) | die verwalteten Repositories mit ihrem Zustand, offene Aufträge und die Entscheidungen, auf die sie warten; Repositories auswählen und einen Pflegeauftrag starten |
| **Aufträge** | jeder Pflegeauftrag: Schritte, Pläne, Vorschläge mit Diff, Ergebnisse und Log; laufende, wartende und bisherige |

Die Website hat in OpenReleasePilot keinen Bereich mehr; sie ist Sache von OpenSitePilot.

Jede Ansicht hat oben rechts höchstens eine Hauptaktion; Aktualisieren (↻) und Öffnen einer Seite (↗)
stehen als Symbole daneben, seltene und nicht umkehrbare Aktionen im ⋯-Menü. Karten öffnen per Klick.

Solange die App läuft, zeigt ein Symbol in der Menüleiste, wie es steht: wie viele Repositories blockiert
sind, Aufmerksamkeit brauchen oder in Ordnung sind, die betroffenen Repositories mit ihrem Hauptgrund, die
offenen Aufträge und ob eine neue Version da ist. Die Zahl neben dem Symbol sind die Entscheidungen, die auf
dich warten. Ein Klick auf ein Repository oder einen Auftrag öffnet ihn im Fenster; **Status aktualisieren**
liest alle Repositories neu. Das Symbol zeigt nur, was die App schon weiß, und liest selbst nichts nach.
Das Fenster zu schließen beendet die App nicht: Sie bleibt in der Menüleiste (ohne Dock-Symbol), laufende
Aufträge gehen weiter. Beendet wird sie über **OpenReleasePilot beenden** im Menü oder ⌘Q.

## Erste Schritte

Die App beginnt mit **Repositories auswählen**: Sie listet die lokalen Git-Klone in deinem Repo-Ordner,
deren Origin auf GitHub liegt, und du hakst an, um welche sie sich kümmern soll; die lokale Kopie trägt
sie gleich mit ein. Mehr ist nicht nötig: keine Website, kein SFTP-Zugang, kein Release. Ein lesender
Statuscheck geht ohne Claude; ein Pflegeauftrag braucht Claude Code und die angemeldete GitHub-CLI, ein
README-Auftrag darüber hinaus nur Git-Zugriff auf das Repository.

## Verwaltete Repositories

Die App führt eine eigene Liste von Repositories (Identität: `owner/name` ohne Rücksicht auf Groß- und
Kleinschreibung, dazu die GitHub-ID). Jedes lässt sich deaktivieren (bleibt lesbar, kein Auftrag nimmt
es) oder aus der Liste nehmen; dabei wird nichts gelöscht, weder auf GitHub noch die lokale Kopie noch
eine Website-Konfiguration. Im Abschnitt **Verwaltung** des Repositorys:

- **Lokale Kopie** (optional): nur fürs Aufräumen lokaler Branches und für Claudes Blick in den Code bei
  der PR-Prüfung. Status und PR-Pflege gehen auch ohne.
- **Release-Regel**: ob die App die nächste Version taggen darf, mit welchem Tag-Präfix (`v` für
  `v1.2.3`, leer für `1.2.3`), und ob ein Workflow des Repositorys zu jedem Tag ein GitHub-Release
  veröffentlicht. Ohne Regel taggt die App nie; sie errät keine Versionierung.
- **Veröffentlicht durch** (nur GitHub): für ein Repository, dessen Release der Tag eines anderen
  verwalteten Repositorys erzeugt, mit demselben Tag, etwa ein öffentliches Repository, in das ein
  privates exportiert. Ein solches Repository hat keine eigene Release-Regel: die App taggt es nie. Ein
  Auftrag, der das Quell-Repository taggt, nimmt es mit (siehe [Release-Ketten](#release-ketten)). Nur
  eine Stufe: die Quelle hat eine Release-Regel und wird nicht selbst durch ein anderes veröffentlicht.

Enthält die lokale Kopie `.openreleasepilot/site.json` (oder das ältere `.openftpupdater/site.json`),
steht das Repository als **Website-Projekt** da. Gepflegt wird es wie jedes andere; die Website selbst
aktualisiert und veröffentlicht OpenSitePilot.

## Zustand der Repositories

**Status aktualisieren** (↻ oben rechts) liest nur bei GitHub: kein Claude, keine Kosten, kein Schreiben, kein
`git fetch`. Gelesen wird beim ersten Öffnen des Bereichs und auf Anforderung, nie im Hintergrund; schlägt
das Lesen fehl, bleibt der ältere Stand sichtbar, aber als alt markiert, und Daten älter als fünf Minuten
gelten als veraltet. Ohne `gh`-Anmeldung lassen sich öffentliche Repositories trotzdem lesen (GitHub
erlaubt dann 60 Anfragen pro Stunde; ein Repository braucht etwa acht).

| Zustand | Wann |
|---|---|
| **Blockiert** | die CI des Default-Branches ist fehlgeschlagen, oder das Repository ist nicht lesbar (GitHub antwortet mit 404 oder verweigert den Zugriff; 404 kann auch „privat ohne Zugriff“ heißen) |
| **Aufmerksamkeit nötig** | offene Dependabot-Pull-Requests, Commits seit dem letzten Release oder Tag, das Repository, durch das es veröffentlicht wird, hat eine neuere Version als sein eigenes neuestes Release (ein fehlgeschlagener oder laufender Export), oder (sobald die Detailansicht sie gelesen hat) Pull Requests mit Konflikt oder roter CI |
| **Unbekannt** | nichts davon, aber nötige Angaben fehlen, sind unvollständig oder alt: CI läuft noch, keine Checks gefunden, unklares Ergebnis, Abfrage fehlgeschlagen |
| **In Ordnung** | alles Nötige wurde erfolgreich und vor Kurzem gelesen, und es gibt nichts zu tun |

Fehlende Release-Angaben allein verhindern „In Ordnung“ nicht; fehlende oder unvollständige Angaben
ergeben es nie. Ein offener Pull Request von einem Menschen ist für sich kein Befund, und ein roter Pull
Request blockiert das Repository nicht. Archivierte Repositories und solche ohne Schreibzugriff bleiben
lesbar, aber die Pflege ist gesperrt. Zustand eines Repositorys und Fortschritt eines Auftrags stehen
getrennt: Ein Repository kann Aufmerksamkeit brauchen, während ein Auftrag schon daran arbeitet.

Über den Karten filtern eine Namenssuche und die Zustände „Aufmerksamkeit“, „Blockiert“ und
„Unbekannt“ die Übersicht; „Alle Treffer“ wählt nur sichtbare Repositories aus, und eine Auswahl
außerhalb des Filters bleibt erhalten und wird als ausgeblendet gezählt.

Die Detailansicht betrifft nur dieses Repository: oben CI, Release und Datenstand in einer Zeile und ein
offener Auftrag als Hinweis mit Link, danach Befunde, die Pull Requests mit Zielbranch, Autor, CI und
Mergebarkeit, die zugeordneten Inhaltsziele mit ihrem zuletzt erreichten Stand und die
Sicherheitsprüfung (mit kritischen oder wichtigen Befunden vor den Pull Requests). Aufklappbar folgen
**Technische Details** (Default-Branch und HEAD-Commit, CI mit ihrem Commit, Release und Tag, Commits seit
dieser Basis, nur wenn sie ein Vorfahr von HEAD ist, sonst „nicht bestimmbar“, Release-Regel, lokale
Kopie) und die **Verwaltung**.

## Sicherheitsprüfung

**Sicherheit prüfen** (in der Übersicht im ⋯-Menü für alle oder die ausgewählten Repositories, oder im Abschnitt
**Sicherheit** eines Repositorys) liest nur GitHub: kein Claude, keine Kosten, kein Schreiben, etwa 10
bis 20 Anfragen je Repository. Sie ist vom Zustand oben getrennt und läuft nur auf Wunsch. Das Ergebnis
steht mit dem Schlimmsten zuerst; was in Ordnung oder nicht lesbar ist, ist eingeklappt.

| Prüfung | Befund, wenn |
|---|---|
| **Direkte Pushes auf den Default-Branch** | weder ein Ruleset noch der klassische Branch-Schutz Pull Requests verlangt (kritisch); der klassische Schutz Admins ausnimmt (Hinweis) |
| **Force-Push und Löschen** | Force-Pushes erlaubt sind (kritisch) oder der Branch gelöscht werden kann (wichtig) |
| **Pflicht-Checks vor dem Merge** | keine Status-Checks verlangt werden (Hinweis) |
| **Secret Scanning**, **Push Protection** | aus (wichtig); offene Secret-Alerts (kritisch) |
| **Dependabot-Alerts**, **Sicherheitsupdates** | Alerts aus (wichtig), offene Alerts (kritisch bei hoch/kritisch, sonst wichtig), Sicherheitsupdates aus (Hinweis) |
| **Code Scanning (CodeQL)** | nicht eingerichtet (Hinweis), offene Alerts nach Schwere |
| **Rechte des Workflow-Tokens** | der `GITHUB_TOKEN` standardmäßig schreiben darf oder Workflows PRs freigeben dürfen (wichtig) |
| **Private Meldung von Sicherheitslücken**, **SECURITY.md** | nur öffentliche Repositories: aus oder fehlt (Hinweis) |
| **Deploy-Keys mit Schreibrecht** | es welche gibt (Hinweis) |

Viele Einstellungen sieht nur ein Admin mit angemeldetem `gh`; was nicht lesbar ist, heißt
**Nicht prüfbar**, nie „in Ordnung“. Wo möglich bietet ein Befund eine Behebung an, für Admins eines
nicht archivierten Repositorys: **Beheben …** zeigt sie mit genau dem, was sich auf GitHub ändert und was
es brechen kann; du wählst, was ausgeführt wird (Behebungen mit Warnung sind zunächst abgewählt). Jede
fügt nur Schutz hinzu, nichts wird gelockert oder entfernt:

- **Nur noch über Pull Requests** / **Force-Push und Löschen verbieten**: legt das Repository-Ruleset
  `OpenReleasePilot: Default-Branch` für den Default-Branch an oder ergänzt es (Pull Requests ohne
  Pflicht-Review, damit du eigene weiter mergen kannst; kein Force-Push, kein Löschen; auch für Admins).
  Eine Regel desselben Typs, die schon da ist, bleibt wie sie ist. Bei einem Website-Projekt warnt die
  App, dass der direkte Push, mit dem OpenSitePilot die Website veröffentlicht, dann abgelehnt wird.
  Private Repositories brauchen für Rulesets GitHub Pro, Team oder Enterprise.
- **Secret Scanning**, **Push Protection**, **Dependabot-Alerts** und **-Sicherheitsupdates**,
  **CodeQL-Default-Setup**, **private Meldung von Sicherheitslücken**: eingeschaltet.
- **Workflow-Token nur lesend**: Standardrechte des `GITHUB_TOKEN` auf Lesen, Workflows dürfen keine PRs
  freigeben; die App nennt die Workflows ohne eigene `permissions:`, die dadurch Schreibrechte verlieren.

Nicht, solange ein offener Auftrag das Repository hält. Was GitHub ablehnt (fehlende Lizenz, fehlender
`gh`-Scope wie `admin:repo_hook` für Dependabot), steht je Behebung im Ergebnis; danach wird das
Repository neu geprüft. Geleakte Secrets, offene Alerts, Pflicht-Checks und Deploy-Keys bleiben bei dir,
mit Link auf die passende GitHub-Seite.

## Pflegeaufträge

Repositories auf ihren Karten auswählen (oder eines öffnen) und **Pflegen …** wählen. Ein Auftrag
besteht aus den Tätigkeiten, die du wählst, jede optional:

- **Pull Requests**: Claude bewertet jeden offenen PR auf den Default-Branch, die App mergt, schließt
  oder bittet Dependabot um einen Rebase (Regeln unten).
- **Aufräumen**: gemergte Branches auf GitHub und in der lokalen Kopie löschen.
- **Release**: die nächste Version taggen, wo eine Release-Regel besteht, sobald die CI genau dieses
  Commits grün ist, und, wenn die Regel es sagt, auf das veröffentlichte GitHub-Release warten. Je
  Auftrag wählst du **Patch**, **Minor** oder **Major** (`1.2.3` → `1.2.4`, `1.3.0`, `2.0.0`), bei
  mehreren Repositories auch je Repository abweichend, und ob
  vorher die Versionsdateien und das CHANGELOG auf die neue Version gebracht werden (Release-Vorbereitung,
  unten). PR-Pflege bedeutet nicht mehr automatisch einen Tag.
- **Inhaltsziele**, die die gewählten Repositories speisen: README/Dokumentation oder Wiki, jeweils mit
  der für diesen Auftrag gewählten Basis (neuestes Release oder aktueller Stand des Default-Branches).
- **Aufgaben aus der Analyse**, z. B. eine Dependabot-Konfiguration als Vorschlags-PR (siehe
  [Analyse, Gedächtnis und Aufgaben](#analyse-gedächtnis-und-aufgaben)).

Die App macht daraus Schritte und führt sie so weit aus, wie es ohne dich geht: lesen, Claudes
Bewertung, Inhalte in ihrem eigenen Arbeitsordner vorbereiten. Bevor sich außerhalb etwas ändert, hält
sie mit einem konkreten Vorschlag an: dem Pflegeplan (jede Aktion lässt sich ausnehmen; Merges, von denen
Claude abrät, die nach den Regeln der App aber zulässig sind, stehen zum Anhaken bereit; sonst lässt sich
nichts hinzufügen) oder einer Inhaltsänderung mit Diff und den Prüfungen der App. Die
Freigabe setzt den Auftrag fort; einzeln starten musst du nichts. Ein Tag ist kein Release: Ein Schritt,
der das Release braucht, wartet darauf (bis zu 30 Minuten je Blick, danach wartet der Auftrag sichtbar,
und **Erneut prüfen** schaut wieder nach). Ein fehlgeschlagener Schritt stoppt die Schritte, die von ihm
abhängen; unabhängige laufen weiter, und ein Teilerfolg wird als solcher angezeigt.

| Zustand des Auftrags | Bedeutung |
|---|---|
| **Geplant** | bereit zum Fortsetzen |
| **In Vorbereitung** / **Läuft** | die App liest, bereitet vor oder führt einen freigegebenen Schritt aus |
| **Freigabe nötig** | ein Plan oder Vorschlag wartet auf deine Entscheidung |
| **Wartet** | wartet auf etwas außerhalb der App: CI, einen Release-Workflow |
| **Blockiert** | geht ohne dich nicht weiter: ein Konflikt, eine geänderte Voraussetzung, ein unterbrochener Lauf |
| **Erfolgreich**, **Teilweise erfolgreich**, **Fehlgeschlagen**, **Abgebrochen** | beendet, mit Grund |

Aufträge liegen im Datenordner der App (eine Datei je Auftrag, nach jeder Änderung atomar geschrieben),
mit Plan, Freigaben, festgehaltenen Quellständen, jedem Ergebnis und dem Log. Ein Neustart vergisst
nichts und startet nichts von selbst: Ein laufender Auftrag gilt danach als unterbrochen, und
**Fortsetzen** prüft zuerst bei GitHub (oder im Wiki, oder im Klon), ob ein begonnener Schreibschritt
stattgefunden hat, bevor irgendetwas anderes läuft; so wird nichts doppelt gemergt, kommentiert,
getaggt, eröffnet oder veröffentlicht. Eine Freigabe gilt nur für den unveränderten Vorschlag: Ein
geändertes Ziel, eine geänderte Repository-Konfiguration, ein neueres Release einer Release-basierten
Quelle oder eine veränderte Datei lässt den Schritt erneut fragen. **Abbrechen** hält nach dem laufenden
Teilschritt an; nichts Erledigtes wird zurückgenommen. Ein Repository oder ein Inhaltsziel gehört
höchstens zu einem offenen Auftrag, und es läuft immer nur ein Auftrag.

Neue Aufträge können die Website nicht mehr enthalten. Ältere offene Aufträge mit einem Website-Schritt
bleiben lesbar; der Schritt wird übersprungen, mit dem Hinweis, dass jetzt OpenSitePilot die Website
aktualisiert und veröffentlicht. Ein dafür schon vorbereiteter lokaler Commit bleibt unveröffentlicht im
Website-Projekt stehen.

## Analyse, Gedächtnis und Aufgaben

Was die App über ein Repository weiß, liegt nur in ihrem Datenordner
(`~/Library/Application Support/OpenReleasePilot/repos/`), nie im Repository. Zu sehen und zu löschen
unter Repository → **Analyse und Gedächtnis**.

- **Analyse**: Bei **Status aktualisieren** liest die App die lokale Kopie, ohne Claude und ohne Netz:
  den zuletzt geholten Default-Branch (`origin/HEAD`, sonst `HEAD`), nur Committetes. Erkannt werden
  Stack (Cargo, npm, Python, Go, Docker), CI-Konfiguration, Dependabot oder Renovate, Tests, CHANGELOG,
  Versionsdateien (dieselben wie bei der Release-Vorbereitung), `CLAUDE.md`/`AGENTS.md`, eigene Skills
  unter `.claude/skills` und `site.json`. Gespeichert mit dem Commit, den sie beschreibt; neu gelesen
  nur, wenn sich der Commit ändert. Ohne lokale Kopie gibt es keine Analyse, und die Aufträge behalten
  ihre bisherigen Standardwerte.
- **Gedächtnis**: Aufträge merken sich je Repository Claudes Bewertungen der PRs, Fehlschläge (mit
  Zähler, wenn sie in mehreren Aufträgen wiederkehren) und was du ausgelassen oder verworfen hast;
  höchstens 60 Notizen, die ältesten fallen weg. Claude bekommt sie bei der PR-Bewertung und bei
  Reparaturen als Material mit, ausdrücklich nicht als Anweisung, ebenso den Hinweis auf
  `CLAUDE.md` und Skills des Repositorys.
- **Aufgaben**: Aus der Analyse bietet der Pflegedialog Aufgaben an, jede mit ihrer Stufe:
  **nur vorschlagen** (ein Hinweis, die App tut nichts), **mit Freigabe** (Vorschlag, dann deine
  Freigabe) oder **automatisch** (nur Lesendes oder Umkehrbares, z. B. Nachweise in der Wegwerf-Kopie).

| Aufgabe | Stufe | Wann |
|---|---|---|
| PRs prüfen und übernehmen | mit Freigabe | vorausgewählt, wenn PRs offen sind |
| Mit Nachweisen prüfen | automatisch | vorausgewählt mit lokaler Kopie und erkannten Tests |
| Release taggen | mit Freigabe | gesperrt mit Begründung, wenn weder CI-Konfiguration noch Checks erkannt sind |
| Versionsdateien und CHANGELOG anpassen | mit Freigabe | vorausgewählt, wenn CHANGELOG.md oder Versionsdateien da sind |
| Dependabot-Konfiguration vorschlagen | mit Freigabe | GitHub, Abhängigkeiten erkannt, weder Dependabot noch Renovate: die App schreibt `.github/dependabot.yml` (wöchentlich, je Manifest-Ordner) im Arbeitsordner, zeigt den Diff und eröffnet nach Freigabe einen PR; mergen tust du |
| CI einrichten | nur vorschlagen | keine CI erkannt |

## Regeln der Repository-Pflege

Die App entscheidet, nicht Claude: Ein Pull Request braucht genau eine gültige Bewertung (grün oder gelb
für einen Merge, eine Begründung fürs Schließen); rote, fehlende, doppelte oder ungültige Bewertungen
lassen ihn offen. Claudes „offen lassen“ ist ein Rat, kein Veto: Einen Pull Request, den Claude grün oder
gelb bewertet und den GitHub als mergebar mit grüner CI meldet, bietet der Plan unter **Zur Wahl** an,
nicht angehakt und mit Claudes Begründung; angehakt wird er nach deiner Freigabe gemergt, mit denselben
Prüfungen wie jeder andere Merge. Der Plan wartet dann auf deine Entscheidung, statt mit „nichts zu tun“
zu enden. Ein Tag, der nur auf einen solchen Merge folgen würde, wird mitgeplant und entfällt wieder,
wenn nichts gemergt wird. Die Bewertung folgt den Regeln des Skills `dependabot-pr`: Risikoklasse (grün, gelb,
rot; unter 1.0 zählt die erste Stelle ungleich null als Major), Release-Notes aus dem offiziellen
Repository, in Dependabot-PRs nur Manifest-, Lock-, Workflow- und Dockerfiles, SHA-Pins von Actions,
zusammengehörige Abhängigkeiten nur gemeinsam, Majors gegen die tatsächliche Nutzung im Code. Das kostet
Claude-Nutzung, je nach Modell und Zahl der PRs etwa ein paar Cent bis rund einen Dollar pro Repository.

**Mit Nachweisen** (standardmäßig an, braucht die lokale Kopie): Statt zu vermuten, baut und testet
Claude jeden PR in einer Wegwerf-Kopie (Worktree der lokalen Kopie, auf dem Commit der Vorschau) und
schreibt bei Bedarf einen kurzen Test (gleiche Hashes für bekannte Eingaben, gleiche Pfade). Die Shell
läuft in der Sandbox von Claude Code: schreiben nur in den Wegwerf-Ordnern und einem Nachweis-Cache im
Temp-Ordner (eigene Paket-Caches, nie die deinen), Netz nur zu Paket-Registries, Zugangsdaten wie
`~/.ssh` und `~/.config/gh` nicht lesbar; `gh`/`glab` laufen nur so weit außerhalb der Sandbox, wie die
Leseregeln es erlauben. Build-Skripte der Abhängigkeiten laufen dabei auf diesem Mac, in dieser Sandbox.
Gepusht wird nichts, die Kopien werden nach der Bewertung gelöscht. Was Claude ausgeführt hat, steht im
Plan als **Nachweis**; ein Major mit grüner CI, dessen Breaking Changes den genutzten Code nachweislich
nicht treffen, ist gelb statt rot. PRs, die Claude offen lässt, stehen als „N brauchen dich“ da, nicht
als „Nichts zu tun“.

**Reparieren lassen:** Lässt Claude ein Update offen, weil der Code angepasst werden muss (eine entfernte
API, der Build bricht), und ist die Anpassung überschaubar, beschreibt Claude sie, und der Plan bietet sie
unter **Reparieren lassen** an, nicht angehakt. Angehakt passt Claude nach der Freigabe den Code in einer
Wegwerf-Kopie auf dem Commit des Updates an, in derselben Sandbox; fertig ist es erst mit grünem Build und
grünen Tests. Die App prüft die geänderten Dateien (keine Pipeline-Dateien, nichts unter `.git`, nichts
gelöscht, Größengrenzen), legt über die API einen Commit auf den Commit des Updates und öffnet daraus einen
neuen PR vom Branch `openreleasepilot/fix-<n>`. Der Branch des Updates bleibt unberührt (Dependabot würde
einen Branch mit fremden Commits nicht mehr neu aufbauen); nach dem Merge der Reparatur schließt der Bot
seinen PR selbst. Gemergt wird beim Reparieren nichts: Den neuen PR bewertet der nächste Pflegeplan wie den
eines Menschen (Diff vollständig, höchstens gelb), mit einem eigenen Claude-Lauf, und gemergt wird nur
nach deiner Freigabe. Eine Reparatur, die Update-, Signatur- oder Zugangsdaten-Code berührt (z. B.
`update.rs`, `verify.rs`), steht nur zur Wahl und wird nie von selbst eingeplant. Ein Abbruch mitten in der
Reparatur wird beim Fortsetzen erkannt: Ist der PR schon offen, gilt sie als erledigt.

**Release-Vorbereitung (Versionsdateien und CHANGELOG):** Mit dieser Option klont der Release-Schritt
zuerst den Default-Branch in den Arbeitsordner der App, genau auf dem Commit, den die Merges dieses
Auftrags hinterlassen haben, und bringt das Repository auf die neue Version, ohne Claude und ohne zu raten:

- `Cargo.toml` (`[package]`, `[workspace.package]`), die eigenen Pakete in `Cargo.lock`, `package.json`,
  `package-lock.json` (oberste Ebene und Wurzelpaket), `tauri.conf.json`, `pyproject.toml` (`[project]`,
  `[tool.poetry]`), `Chart.yaml` (`version`, `appVersion`) und `VERSION`/`version.txt`, wo immer sie
  liegen (nicht in `node_modules`, `vendor`, `target`, `dist`, `build`, `.venv`), aber nur dort, wo genau
  die Version des letzten Tags steht;
- eine README im Wurzelordner nur in Zeilen, die das Repository nennen (etwa `MyApp-1.2.3.pkg` in einem Repository namens MyApp),
  und dort nur die Version als Ganzes (nicht `11.2.3` oder `1.2.31`);
- `CHANGELOG.md` im Wurzelordner: Ein gefüllter Abschnitt „Unreleased“ wird zur Version, darüber ein leerer
  neuer; sonst ein neuer Abschnitt `## [x.y.z] - Datum` über dem neuesten, mit den Commits seit dem letzten
  Tag entlang des Default-Branches (ein Merge mit dem Titel seines PRs, Release-Commits ausgelassen). Ein
  CHANGELOG, das die Version schon hat, bleibt unverändert; ohne `CHANGELOG.md` wird keins angelegt.

Das Ergebnis kommt als eigene Entscheidung mit Diff und den Prüfungen der App. Freigegeben öffnet die App
einen PR vom Branch `openreleasepilot/release-<tag>` (genau die freigegebenen Dateien auf diesem Commit,
über die API), mergt ihn bei grüner CI, solange sich der Default-Branch nicht bewegt hat, und taggt den
Merge-Commit, sobald auch dessen CI grün ist; ein Merge auf etwas anderes heißt: kein Tag. Ohne passende
Datei folgt der Tag direkt. **Verwerfen** heißt: kein Release-PR und kein Tag; **Neu vorbereiten** liest
den Stand neu.

Direkt vor jedem Schreiben prüft die App erneut:

- **Merge:** noch offen und kein Entwurf, gleicher Head-Commit, gleicher Zielbranch und gleiches
  Repository, laut GitHub mergebar, CI des Head-Commits grün; GitHub bekommt den erwarteten Commit mit,
  ein Push dazwischen lässt es ablehnen. Merge-Commit, wo das Repository es erlaubt.
- **Schließen:** noch offen, gleicher Head und gleiches Ziel; ein von Claude genannter Ersatz-PR muss noch
  offen oder gemergt sein. Die Begründung geht als Kommentar an den PR; Kommentar und Schließen werden
  einzeln gemeldet.
- **Rebase:** noch offen, wirklich von Dependabot, gleicher Head, weiterhin Konflikt oder Rückstand; dann
  `@dependabot rebase`. Ein Dependabot-PR, den ein Merge desselben Auftrags in einen Konflikt gebracht
  hat, bekommt das statt des Merges, wie der Plan es sagt.
- **Tag:** die nächste Version (wie gewählt: Patch, Minor oder Major) des höchsten `x.y.z`-Tags mit dem Präfix der Regel, nur wenn jeder
  geplante Merge des Repositorys gelungen ist, der Default-Branch genau dort steht, wo die Merges dieses
  Auftrags ihn hinterlassen haben, die Release-Linie unverändert ist, es Commits seit dem letzten Tag gibt
  und die CI genau dieses Commits erfolgreich war (die App wartet bis zu 30 Minuten, danach wartet der
  Auftrag und kann erneut nachsehen). Rot, unklar, abgebrochen oder **gar keine Checks** heißt: kein Tag.
  Direkt vor dem Schreiben, nach dem Warten, prüft die App die Release-Linie erneut: Ein höherer Tag oder
  eine verschobene Basis macht die geplante Version ungültig. Bestehende Tags werden nie verschoben; das
  erste Release wird von Hand getaggt.
- **Gemergte Branches löschen:** auf GitHub nur, solange der Branch auf demselben Commit steht, nicht
  geschützt und nicht der Default-Branch ist, kein offener PR aus ihm kommt oder auf ihn zielt und sein
  Nachweis bei GitHub noch gilt (alle Commits im Default-Branch, oder sein PR gemergt; ein Head-Branch
  eines Merges im selben Auftrag nur, wenn dieser Merge gelungen ist). Lokal nur in einer Kopie, die
  noch zu dem Repository gehört, nie ein Branch, der in einem Worktree ausgecheckt ist, und nur mit
  Bestätigung durch GitHub: Ob er enthalten ist, prüft die App gegen GitHubs aktuellen Default-Branch,
  nicht gegen das womöglich alte `origin/<default>` der Kopie. Hat der Plan für die Kopie
  `git fetch --prune origin` angekündigt und das ist nicht gelungen, löscht die App lokal nichts.

Jede Aktion endet als erledigt, übersprungen, fehlgeschlagen oder abgebrochen, mit Grund. Wo die
GitHub-API keine Vorbedingung kennt (Schließen, Branch löschen), bleibt ein kurzes Fenster zwischen
letzter Prüfung und Schreiben.

## Release-Ketten

Ein Release geht oft in anderen Repositories weiter: ein privates Repository exportiert seinen Kern in
ein öffentliches, das aus demselben Tag eigene Images und ein eigenes Release baut, und das öffentliche
speist ein Wiki. Mit **Veröffentlicht durch** am öffentlichen Repository nimmt ein
Auftrag, der das private taggt, die ganze Kette:

1. Pflegeplan und Tag im Quell-Repository, wie gewohnt.
2. **Release `<Repository>` (aus `<Quelle>`)**: die App schreibt dort nichts. Sie wartet auf das Release
   mit demselben Tag (bis zu 30 Minuten je Prüfung). Ein fehlgeschlagener Lauf in der Quelle am getaggten
   Commit (etwa der Export) oder ein fehlgeschlagener Lauf zum Tag im Repository selbst blockiert den
   Schritt, mit Link auf den Lauf; ein Tag wird nie verschoben, nach der Korrektur braucht es also eine
   neue Version.
3. README- und Wiki-Schritte, die das veröffentlichte Repository speist, warten auf dieses
   Release, nicht auf den Tag der Quelle, und bauen darauf auf.

Das veröffentlichte Repository wird in diesem Auftrag nicht gepflegt (seine Pull Requests, falls es
welche gibt, in einem eigenen Auftrag), und es zusammen mit seiner Quelle zu wählen, lehnt die App ab.
Ein README- oder Doku-Ziel darin lehnt sie ebenfalls ab, weil der Export dessen Dateien ersetzt: README
und Doku im Quell-Repository pflegen; das Wiki ist ein eigenes Repository und bleibt ein Ziel.

## Inhaltsziele: README, Dokumentation, Wiki

Ein Inhaltsziel sagt, welche Inhalte wo auf welcher Grundlage aktualisiert werden: Art, Name, die
Repositories, die es beschreibt (seine Quellen), das Repository mit den Dateien (oder dessen Wiki),
Zielbranch, erlaubte Pfade, der Auftrag an Claude und optional Skill, Modell und Prüfbefehl. Ziele legst
du in der Detailansicht eines Repositorys unter **Zugeordnete Inhalte** an. Jedes Ziel ist nur für
bestimmte Quellstände aktuell: Eine README auf dem Stand von Release 1.3.0 sagt nichts über das Wiki oder
die Website, und „vorbereitet“, „PR erstellt“, „gemergt“ und „veröffentlicht“ sind verschiedene
Ergebnisse.

Eine Änderung vorbereiten, in einem Arbeitsordner der App (nie in deinen Klonen):

1. Die Quellen werden festgehalten: ihr neuestes Release oder der HEAD ihres Default-Branches.
2. Das Ziel wird geklont (Dokumentation: sein Branch; Wiki: `https://github.com/<owner>/<name>.wiki.git`),
   jede Quelle daneben auf ihrem festgehaltenen Commit ausgecheckt.
3. Claude Code bearbeitet das Ziel. Es darf die Quellen lesen und Dateien bearbeiten, sonst nichts: keine
   Shell, kein Web, nichts in `.git` oder `.github`. Der Prompt nennt, was sich in jeder Quelle seit dem
   Stand geändert hat, für den das Ziel zuletzt veröffentlicht wurde, und sagt Claude, dass Texte in den
   Quellen Material sind, keine Anweisungen.
4. Die App entscheidet, was zählt: nur Dateien unter den erlaubten Pfaden, keine Löschungen, keine Links,
   Größengrenzen, und Gits eigene Konfiguration unverändert; alles andere verwirft die ganze Änderung.
   Dann ihre Prüfungen: keine Konfliktmarker, keine leeren Dateien, kein relativer Markdown-Link, den die
   Änderung kaputt macht, und der Prüfbefehl des Ziels, falls gesetzt (im Arbeitsordner, ohne GitHub-Token).
5. Der Vorschlag zeigt Quellen, Ausgangs-Commit, Prüfungen, Dateien, Claudes Zusammenfassung und den
   vollständigen Diff.

Nach der Freigabe, und nur mit genau den freigegebenen Dateien:

- **README/Dokumentation:** Die App prüft, dass niemand diese Dateien seit dem Vorschlag auf dem
  Zielbranch geändert hat, legt dann über die GitHub-API einen Commit auf dem Ausgangs-Commit des
  Vorschlags an, einen neuen Branch `openreleasepilot/<auftrag>-<ziel>-<hash>` und einen Pull Request in
  den Zielbranch; kein Hook und keine Einstellung des Arbeitsordners wirkt dabei mit. Nie ein direktes
  Schreiben auf den Default-Branch. Den Pull Request erledigt dann die normale PR-Pflege oder du, oder,
  wenn das Ziel es sagt, die App mergt ihn, sobald die CI genau seines Commits grün ist. Als
  veröffentlicht gilt er, sobald er gemergt ist.
- **Wiki:** Ein Wiki hat keine Pull Requests. Die App klont es frisch, prüft, dass es noch auf dem
  Ausgangs-Commit des Vorschlags steht, committet die freigegebenen Dateien und pusht ohne Force. Hat
  jemand das Wiki inzwischen geändert, hält sie an und überschreibt nichts.

Inhalte lassen sich auch ohne neues Release aktualisieren: Mit der Basis „aktueller Stand“ hält der
Auftrag den HEAD des Default-Branches fest und lässt das Ziel nicht mangels neuer Version aus.

## GitLab

GitLab-Projekte, auf gitlab.com oder einer eigenen Instanz, pflegt die App nach denselben Regeln wie
GitHub-Repositories. Die Startansicht zeigt je Dienst eine Karte: GitHub und jede GitLab-Instanz mit
Anmeldestatus, einem Quadrat je Repository und den Zahlen, die Aufmerksamkeit brauchen. Ein Klick
schiebt die Karten zu einer Leiste zusammen und öffnet darunter die Übersicht dieses Dienstes.

- **Anmeldung:** über die GitLab-CLI, `glab auth login --hostname <instanz>`, oder mit einem eigenen
  Zugriffstoken (Scope `api`) unter **Einstellungen → Zugänge → Instanz mit Token hinzufügen**; dort auch
  http für eine eigene Instanz ohne TLS. Ein eigenes Token liegt nach Wahl im Schlüsselbund oder in einer
  nur für dich lesbaren Datei im Datenordner der App und geht vor dem von `glab`. Claude Code bekommt es nie.
  „Status aktualisieren“ fragt erneut, eine Anmeldung danach zählt also sofort.
- **Identität:** ein GitLab-Projekt heißt in der App `host/gruppe/…/projekt`, z. B.
  `gitlab.example.com/werk/tools/app`. Lokale Klone mit diesem Origin (HTTPS oder SSH) erscheinen unter
  „Repositories auswählen“; ohne Klon dort den Namen oder die Adresse des Projekts eingeben.
- **Status:** die neueste Pipeline genau des Commits (wartet sie auf einen manuellen Job oder wurde sie
  übersprungen, gilt sie als unklar, nicht als grün), offene Merge Requests, Releases und Tags.
- **Pflege:** Claude bewertet die Merge Requests nur lesend mit `glab`. Gemergt wird über die API mit der
  Merge-Methode des Projekts und dem erwarteten Commit (`sha`), sodass ein Push dazwischen den Merge
  ablehnt. Renovate gilt als Update-Bot; einen Renovate-MR mit Konflikt baut Renovate nach dem Label
  „rebase“ neu auf. Aufräumen, Tag auf grüner Pipeline und das Warten auf das GitLab-Release laufen wie
  bei GitHub.
- **Inhalte:** README und Dokumentation als ein Commit auf einem neuen Branch genau vom Ausgangs-Commit
  und ein Merge Request; das Wiki über sein eigenes Git-Repository (`<projekt>.wiki.git`), Push ohne
  Force. `.gitlab-ci.yml` und `.gitlab/` sind, wie `.github`, für Inhaltsziele und Claude nie schreibbar,
  weil ein gepushter Branch sie ausführt.
- **Sicherheitsprüfung:** Schutz des Default-Branches (direkte Pushes, Force-Push), grüne Pipeline als
  Merge-Voraussetzung, Push-Regel gegen Secrets (GitLab Premium), Secret Detection in der
  `.gitlab-ci.yml`, Renovate-Konfiguration, Zugriff fremder CI-Job-Tokens, Deploy-Keys mit Schreibrecht,
  SECURITY.md bei öffentlichen Projekten. Was der Tarif oder die Rechte nicht zeigen, steht als „nicht
  prüfbar“ da. Behebungen gibt es für Maintainer: Default-Branch schützen, Force-Push sperren, grüne
  Pipeline verlangen, Push-Regel gegen Secrets, Job-Token-Zugriff beschränken.

## OpenSitePilot: Website

OpenSitePilot ist die eigene App für das, was früher der Bereich „Website (optional)“ von
OpenReleasePilot war: eine statische Website, deren Projektdatei `.openreleasepilot/site.json` Build und
beobachtete Produkte beschreibt (siehe [Projektdatei](#projektdatei-der-website-openreleasepilotsitejson)).
Eingerichtet wird sie unter **Einstellungen → Website und Veröffentlichung**: Website-Repo aus der Liste
wählen (der Projektordner ergibt sich aus dem lokalen Klon; gibt es keinen, klont die App das Repo in den
Repo-Ordner), Repo-Ordner und SFTP-Ziel; unter **App** steht die Suche nach Updates. Ihre Ansicht zeigt
die vier Schritte, einzeln oder nacheinander:

| Schritt | Was passiert | Ändert etwas? |
|---|---|---|
| **Prüfen** | Projektdatei, Werkzeuge (git, node, npm, claude, gh), sauberes Arbeitsverzeichnis und Remote-Stand, neuestes GitHub-Release jedes beobachteten Produkts gegen den dokumentierten Stand, SFTP-Anmeldung und Zielordner, Live-Seite, letzter Upload | nein |
| **Inhalte aktualisieren** | `git pull --ff-only`, dann läuft Claude Code headless im Projekt mit dem Skill des Projekts und der Liste veralteter Produkte (plus optionaler Zusatzanweisung), danach Build und Commit | Dateien im Projekt, ein Commit |
| **Veröffentlichen** | Frischer Build, `git push`, dann nur neue oder geänderte Dateien per SFTP; aus dem Build entfernte Dateien werden auf dem Server gelöscht | Server, Remote-Repository |
| **Live-Seite prüfen** | Lädt jede Datei des letzten Uploads von der Live-Seite und vergleicht SHA-256 mit dem Build | nein |

![Wann passiert was wo: Precheck, Update-Aufgaben in Claude Code mit ihren Skills, Build und Commit, Upload per SFTP, Verify gegen die Live-Seite](docs/diagrams/openreleasepilot-flow.de.png)

<sub>Quelle der Grafik: [docs/diagrams/openreleasepilot-flow.de.html](docs/diagrams/openreleasepilot-flow.de.html) (auch als SVG).</sub>

**Aktualisieren und veröffentlichen** führt alle vier Schritte nacheinander aus, bis zur Live-Seite, und
hält beim ersten Fehler an. Schlägt eine Release-Abfrage fehl, ist der Stand unbekannt, nicht „aktuell“:
„Inhalte aktualisieren“ hält an und damit auch der gesamte Ablauf vor dem Veröffentlichen, statt auf
unvollständiger Grundlage zu veröffentlichen.

Neue Releases der Produkte bemerkt OpenSitePilot selbst: „Prüfen“ fragt GitHub nach dem neuesten Release
jedes Produkts. Eine direkte Verbindung zu OpenReleasePilot gibt es nicht. GitHub liest OpenSitePilot mit
der Anmeldung der GitHub-CLI (`gh`); ein eigenes Token lässt sich hier nicht hinterlegen. Ein Symbol in
der Menüleiste hat OpenSitePilot nicht: Das Fenster zu schließen beendet die App.

**Produkt zur Website hinzufügen** (Knopf in OpenSitePilot): eines deiner GitHub-Repositories wählen;
„Produktseite erstellen“ lässt Claude Code es mit dem `addProductSkill` des Projekts aufnehmen
(Produktseite, Listen, Icon, Ablaufgrafik, Eintrag in `site.json`), danach baut und committet die App.
Live geht es mit „Veröffentlichen“.

OpenSitePilot listet die Produkte aus `site.json` mit je einer Checkbox. „Prüfen“ und „Inhalte
aktualisieren“ kümmern sich nur um ausgewählte Produkte: Abgewählte fragt „Prüfen“ nicht ab, und Claude
bekommt sie als „nicht ausgewählt, daran nichts ändern“ genannt. Die Auswahl merkt sich die App in ihren
Einstellungen, nicht im Website-Repository; welche Repositories OpenReleasePilot verwaltet, ändert sie nicht.

**Übernahme beim ersten Start:** Hat OpenSitePilot noch keine eigenen Einstellungen, übernimmt es die von
OpenReleasePilot: Projektordner, Repo-Ordner, SFTP-Einstellungen samt bestätigtem Server-Schlüssel, die
Wahl der Update-Aufgaben, abgewählte Produkte und ob beim Start nach Updates gesucht wird. Die Manifeste
der Uploads (`uploads/`) werden kopiert, der nächste Upload bleibt also inkrementell. Das SFTP-Passwort
wandert beim ersten Gebrauch aus dem Schlüsselbundeintrag von OpenReleasePilot in den von OpenSitePilot;
macOS fragt dabei unter Umständen einmal, ob OpenSitePilot diesen Eintrag verwenden darf.
OpenReleasePilot lässt seine alten Website-Angaben unverändert in seiner `config.json` stehen, die
Übernahme gelingt also auch später noch. War dort eine Website eingerichtet, zeigt OpenReleasePilot
einmal einen Hinweis, dass die Website jetzt in OpenSitePilot liegt, mit einem Knopf zur Release-Seite.

## Geändert gegenüber früheren Versionen

- Neu: Analyse der lokalen Kopie, Gedächtnis je Repository und Aufgaben daraus im Pflegedialog
  (siehe [Analyse, Gedächtnis und Aufgaben](#analyse-gedächtnis-und-aufgaben)). Ohne CI ist **Release**
  schon im Dialog gesperrt, nicht erst beim Tag. Ein Repository aus der Verwaltung zu entfernen löscht
  auch seine Analyse und sein Gedächtnis.
- Seit 0.5.0: Die Website ist in eine eigene App gewandert, **OpenSitePilot**, die im selben Release
  erscheint (siehe [OpenSitePilot](#opensitepilot-website)). OpenReleasePilot pflegt nur noch
  Repositories und Inhaltsziele (README, Dokumentation, Wiki): kein Bereich Website, kein Website-Schritt
  in Aufträgen, keine SFTP-Einstellungen, keine Website-Produkte an den Repositories; ein Klon mit
  `site.json` heißt **Website-Projekt**. OpenSitePilot übernimmt beim ersten Start die Website- und
  SFTP-Einstellungen, die Upload-Manifeste und das SFTP-Passwort. Ältere offene Aufträge überspringen
  ihren Website-Schritt.
- Die verwalteten Repositories sind eine eigene Liste der App, ausgewählt aus den lokalen Klonen.
  Produkt-Repositories aus `site.json` erscheinen nicht mehr von selbst. Startansicht ist
  **Repositories**.
- PR-Pflege, Aufräumen und Release sind getrennt wählbar; Mergen bedeutet keinen Patch-Tag mehr, und ein
  Tag braucht eine in der App eingerichtete Release-Regel (das `tagPrefix` aus `site.json` allein genügt
  nicht).
- Die Pflege läuft als gespeicherter Auftrag in **Aufträge** (bis 0.3.4 „Läufe“): Der Plan wird dort freigegeben und übersteht
  einen Neustart.
- Lokale Branches löscht die App nur mit Bestätigung durch GitHub und nicht nach einem fehlgeschlagenen
  Abgleich der Kopie.
- Eine fehlgeschlagene Release-Abfrage hält die Aktualisierung der Website (und **Aktualisieren und veröffentlichen**) an, statt als
  „nichts Neues“ zu gelten.
- Seit 0.3.0: Ein Tag braucht eine CI, die für genau den getaggten Commit erfolgreich war; Repositories
  ohne CI bekommen keinen automatischen Tag.
- Seit 0.4.0: GitLab-Projekte (gitlab.com und eigene Instanzen, Anmeldung über `glab`); die
  Startansicht zeigt je Dienst eine Karte. Siehe [GitLab](#gitlab).
- Seit 0.4.1: Für GitHub und GitLab geht auch ein eigenes Token statt `gh` oder `glab`, abgelegt im
  Schlüsselbund oder in der App (**Einstellungen → Zugänge**); eigene GitLab-Instanzen auch über http.
  Der leere `gitlab.com`-Eintrag, den `glab` immer anlegt, zählt nicht mehr als Instanz.
- Seit 0.3.5: „Einrichtung“ heißt **Einstellungen**, „Läufe“ heißt **Aufträge**, die Website-Schritte
  heißen Prüfen, Inhalte aktualisieren, Veröffentlichen und Live-Seite prüfen. „Verbindung testen“
  speichert nichts mehr; erst „Sichern“ übernimmt die Einstellungen, auch einen dabei bestätigten
  Server-Schlüssel. Eine ungültige Kostengrenze (0, negativ, kein Betrag) wird abgelehnt, statt die
  Grenze still zu entfernen; nur ein leeres Feld entfernt sie. Ein Branch, der beim Löschen schon fehlt,
  gilt als erledigt und macht einen Auftrag nicht mehr „teilweise erfolgreich“. Ein abgeschlossener
  Auftrag wird über „Auftragsverlauf löschen …“ mit Rückfrage entfernt.

## Repositories wählen

**Repositories auswählen** listet die Git-Klone direkt im Repo-Ordner, deren `origin` ein
GitHub-Repository ist, mit ihrem Pfad, und markiert, was schon verwaltet wird oder ein
**Website-Projekt** ist (mit `site.json`). Klone eines Wikis fehlen (Wikis sind Inhaltsziele), und ein
doppelt geklontes Repository steht nur einmal da, mit dem Ordner, der so heißt wie das Repository. Für
eine Auswahl lässt sich auch ein anderer Ordner wählen.
Aufgenommen wird ein Repository so, wie der Origin des Klons es nennt, mit dem Klon als lokaler Kopie;
GitHubs Name und ID kommen dazu, wenn GitHub sich fragen lässt (ein privates Repository ohne
`gh`-Anmeldung wird allein aus dem Origin aufgenommen). Für ein Repository ohne lokalen Klon
`owner/name` eingeben; es muss auf GitHub existieren.

- **Repo-Ordner** (Einstellungen): hier sucht die Auswahl nach Klonen, und hier legt die App neue an;
  standardmäßig `~/Documents/Code`. War keiner gesetzt, nahm die App bisher den Elternordner des
  Website-Projekts; diesen Ordner schreibt sie beim Laden jetzt als eigenen Repo-Ordner fest, es zieht
  also nichts um.
- **Website-Repo**: steht jetzt in den Einstellungen von OpenSitePilot (siehe
  [OpenSitePilot](#opensitepilot-website)).

## Installation

`OpenReleasePilot-<version>.pkg` aus den
[Releases](https://github.com/bl0rb/OpenReleasePilot/releases) laden (Apple Silicon), dazu
`OpenSitePilot-<version>.pkg` nur, wenn du eine Website veröffentlichst, und jedes mit der
`.sha256`-Datei daneben prüfen:

```bash
shasum -a 256 -c OpenReleasePilot-*.pkg.sha256
shasum -a 256 -c OpenSitePilot-*.pkg.sha256
```

Jedes Paket installiert seine App (`OpenReleasePilot.app`, `OpenSitePilot.app`) nach `/Applications` und
ersetzt eine installierte Version. Die Pakete sind nicht signiert, macOS blockiert sie deshalb beim ersten
Öffnen; einmal unter **Systemeinstellungen → Datenschutz & Sicherheit → Dennoch öffnen** erlauben oder im
Terminal installieren:

```bash
sudo installer -pkg OpenReleasePilot-0.5.0.pkg -target /
sudo installer -pkg OpenSitePilot-0.5.0.pkg -target /
```

Dieselben Pakete lassen sich über Microsoft Intune als „macOS-App (PKG)“ verteilen; dann in den
Einstellungen „Beim Start nach neuen Versionen suchen“ abschalten.

Bis Version 0.1.3 hieß die App OpenFTPUpdater-AI. OpenReleasePilot ersetzt `OpenFTPUpdater-AI.app`
nicht; die alte App nach dem Umstieg löschen (Einstellungen und Passwort werden übernommen, siehe unten).

## Updates der App

Beide Apps aktualisieren sich selbst aus demselben Release dieses Repositories, jede nur mit ihrem eigenen
Paket (`<App>-<version>.pkg`); beide tragen dieselbe Versionsnummer. Wie bei ClaudeKeyZapper: Beim Start
fragt die App einmal still das neueste Release auf GitHub ab; manuell geht es über **Nach Updates suchen**
unten in der Seitenleiste. Ist eine neuere Version da,
erscheint oben ein Banner. **Installieren** lädt die `.pkg` von `github.com`, prüft sie gegen die
SHA-256-Prüfsumme, die GitHub für das Release-Asset veröffentlicht, und gegen die minisign-Signatur des
Releases (`<Paket>.minisig`, geprüft mit dem in die App eingebauten öffentlichen Schlüssel,
[desktop/update-key.pub](desktop/update-key.pub)), installiert sie hinter der Administrator-Abfrage von
macOS und startet die App als neue Version neu; ohne Prüfsumme oder gültige Signatur wird abgelehnt.
Scheitert die Installation, öffnet sich stattdessen der macOS-Installer. Nicht während ein Schritt oder
Auftrag läuft. Beta-Versionen (`X.Y.Z-beta.N`) gibt es nur über
**Jetzt nach Beta-Version suchen** unter **Einstellungen → App** oder wenn schon eine Beta installiert ist. Entwicklungs-Builds prüfen nicht.

## Voraussetzungen

- macOS (entwickelt unter macOS 27; der Code ist plattformübergreifendes Tauri, Windows ist ungetestet)
- Auf dem Rechner: `git` mit Zugriff auf deine Repositories, angemeldetes
  [Claude Code](https://claude.com/claude-code), für Pflegeaufträge die angemeldete GitHub-CLI `gh` (sie
  hebt auch das GitHub-API-Limit von 60 auf 5000 Anfragen pro Stunde; das Token nutzt die App selbst,
  gibt es aber nie an Claude Code), für GitLab-Projekte die
  angemeldete GitLab-CLI `glab` (`brew install glab`, dann `glab auth login --hostname <instanz>`). Statt
  der CLIs geht je Dienst auch ein eigenes Token unter **Einstellungen → Zugänge** von OpenReleasePilot
- Nur für OpenSitePilot: die angemeldete GitHub-CLI `gh` (ihr Token geht als `GITHUB_TOKEN` an den Build
  der Website, nie an Claude Code), Node.js/npm (oder was der Build braucht) und ein SFTP-Zugang zum Webspace

Beide Apps starten aus dem Finder und fragen einmal die Login-Shell nach dem `PATH`, damit
Werkzeuge aus nvm, Homebrew oder `~/.local/bin` gefunden werden.

## Projektdatei der Website `.openreleasepilot/site.json`

Liegt im Website-Repository und beschreibt Build und beobachtete Produkte; gelesen wird sie von
OpenSitePilot. Projekte, die vor der
Umbenennung von OpenFTPUpdater-AI eingerichtet wurden, behalten ihre `.openftpupdater/site.json`; sie
wird gelesen, solange die neue Datei fehlt.

```json
{
  "name": "example.com",
  "url": "https://example.com",
  "build": "npm run build",
  "output": "dist",
  "addProductSkill": "product-add",
  "tasks": [
    { "id": "texts", "label": "Texte", "skill": "site-texts", "effort": "high" },
    { "id": "screenshots", "label": "Screenshots", "skill": "site-screenshots", "effort": "medium" }
  ],
  "failOnOutput": ["[releases]"],
  "products": [
    { "id": "app", "name": "App", "repo": "owner/app", "tagPrefix": "v", "documented": "1.2.0", "localPath": "../app" }
  ],
  "claude": {
    "model": "opus",
    "effort": "high",
    "maxBudgetUsd": 15,
    "allowedTools": ["Bash(node scripts/screenshots/*)"],
    "disallowedTools": []
  }
}
```

| Feld | Bedeutung |
|---|---|
| `url` | Öffentliche Adresse; dort liegt die Wurzel der Build-Ausgabe (für Precheck und Verify) |
| `build`, `output` | Build-Befehl (läuft mit `/bin/sh -c` im Projekt) und sein Ausgabeordner (ein Unterordner des Projekts: kein absoluter Pfad, kein `..`) |
| `tasks` | Update-Aufgaben, in der App als Häkchen wählbar; je Aufgabe `id`, `label`, `skill` (`.claude/skills/<name>/SKILL.md`), optional `description`, `default` (gewählt, solange nicht abgewählt; Standard `true`), `model` und `effort` |
| `skill` | Einzelner Update-Skill für Seiten ohne `tasks` |
| `addProductSkill` | Claude-Code-Skill, der ein Repository als neues Produkt aufnimmt |
| `failOnOutput` | Zeilen der Build-Ausgabe, die den Build trotz Exit-Code 0 scheitern lassen, z. B. eine Warnung, dass Versionen auf alte Werte zurückgefallen sind |
| `products[].repo` | GitHub-Repository, dessen neuestes Release (ohne Pre-Releases) beobachtet wird |
| `products[].documented` | Release, den die Inhalte beschreiben; der Skill setzt ihn nach dem Update hoch |
| `products[].tagPrefix` | `v` bei Tags wie `v1.2.0`, leer bei `1.2.0` |
| `products[].localPath` | Lokaler Klon relativ zum Projekt; Claude liest ihn über `git -C`, ändert ihn nie |
| `claude.model`, `claude.effort` | Standard-Modell-Alias (`opus`, `sonnet`, `haiku`, `fable`) und Effort (`low` … `max`) für jeden Claude-Lauf von OpenSitePilot |
| `claude.maxBudgetUsd` | Kostengrenze je Claude-Lauf von OpenSitePilot (`--max-budget-usd`) |
| `claude.allowedTools` | Ergänzt die Freigabeliste der App (siehe unten); CLI-Flags und ein uneingeschränktes `Bash` werden abgelehnt |

Das eigentliche Wissen steckt im Skill: wo die Produkttexte stehen, wie Screenshots entstehen,
welche Aussagen nie gemacht werden dürfen. Claude liest ihn bei jedem Update.

## Update-Aufgaben und Modelle

Jede gewählte Aufgabe läuft als eigene Claude-Code-Sitzung, in der Reihenfolge von `tasks`, mit
eigenem Skill, Modell und Effort; die App committet am Ende einmal. Kleine Aufgaben-Skills halten
jeden Lauf fokussiert, und Regeln für alle (Quellen, Stil, was nie behauptet werden darf) gehören in
die `CLAUDE.md` des Projekts, die Claude Code bei jedem Lauf liest. In der App hat jede Aufgabe ein
Häkchen und eine Modellwahl; beides bleibt gespeichert.

Das Modell ist ein Claude-Code-Alias, der auf das neueste Modell seiner Familie zeigt, soweit das
installierte Claude Code es kennt. Deshalb führt die App vor Claude-Läufen `claude update` aus
(höchstens alle sechs Stunden); das Log zeigt die Version und am Anfang jedes Laufs das genaue Modell:

| Alias | Modell (Okt. 2026) | API-Preis je 1 Mio. Token (Eingabe / Ausgabe) | Einsatz |
|---|---|---|---|
| `opus` | Claude Opus 5.5 | 4 $ / 20 $ | Standard: öffentliche Produktaussagen in zwei Sprachen brauchen Genauigkeit |
| `sonnet` | Claude Sonnet 5.5 | 2 $ / 10 $ | Einen Versuch wert bei mechanischen Aufgaben wie Screenshots und Grafiken |
| `haiku` | Claude Haiku 5.5 | siehe Preisliste von Anthropic | Kleine, klar per Skript vorgegebene Aufgaben |
| `fable` | Claude Fable 5.1 | 10 $ / 50 $ | Nur wenn Opus nicht reicht, etwa bei einer neuen Produktseite |

Effort ist der erste Hebel: `high` für Texte und Konsistenzprüfung, `medium` für Screenshots und
Grafiken. Bei Anmeldung mit einem Claude-Abo sind die angezeigten Kosten ein rechnerischer API-Preis;
die Läufe zählen stattdessen gegen die Nutzungsgrenzen des Abos.

Pflegeaufträge von OpenReleasePilot nutzen das im Auftrag gewählte Modell (ein Inhaltsziel kann ein
eigenes setzen), sonst die Vorgabe unter **Einstellungen → Allgemein → Ausführungsregeln**, sonst den
Standard von Claude Code; die Kostengrenze je Lauf ebenso. `claude.model` und `claude.maxBudgetUsd` der
Website gelten dafür nicht mehr. Die Kostengrenze ist ein Betrag in
USD größer als 0 (Komma oder Punkt); ein leeres Feld heißt: keine eigene Grenze der App.

## Was Claude darf

Jeder Lauf von Claude Code ist headless (`claude -p`) und kann nicht nachfragen; was nicht erlaubt ist,
wird abgelehnt, und abgelehnte Aufrufe stehen nach dem Lauf im Log. Jede Art von Lauf hat eigene Rechte:

- **PR-Prüfung:** nur lesend: der Klon, `gh pr view|diff|checks|list`, `gh release`, lesende
  `gh api`-Aufrufe, das Web für Release-Notes. Keine Änderungen, kein `gh pr merge|close|comment`, keine
  schreibenden Aufrufe. Mergen, schließen und kommentieren macht die App.
- **MR-Prüfung (GitLab):** ebenso nur lesend: `glab mr view|diff|list`, `glab release`, lesende
  `glab api projects/…`-Aufrufe; kein `glab mr merge|close|note|approve|update|rebase`, kein `glab api`
  mit Methode oder Feldern.
- **Inhalte (README, Dokumentation, Wiki):** Dateien im Arbeitsordner der App (dem Klon des Ziels) lesen
  und bearbeiten, die festgehaltenen Quellen daneben lesen; Skills. Keine Shell, kein Web, nichts in
  `.git`, `.github`, `.gitlab` oder `.gitlab-ci.yml`. Was zählt, entscheidet danach die App (siehe Inhaltsziele).
- **Website-Update (OpenSitePilot):** wie folgt.

Für die Website läuft Claude Code in OpenSitePilot als `claude -p … --permission-mode acceptEdits` im
Projektordner:

- Dateien im Projekt lesen, durchsuchen, bearbeiten und schreiben; Skills nutzen
- git nur lesend, im Projekt und in anderen Repositories über `git -C <Pfad> log|show|diff|tag|describe|ls-tree|fetch --tags`
- `gh release list|view`, `grep`, `ls`, der Build-Befehl des Projekts
- was `claude.allowedTools` in site.json ergänzt

Immer abgelehnt: `git push`, `git commit`, `git reset`, `git checkout`, `rm`. Committen,
pushen und hochladen macht die App. Abgelehnte Aufrufe stehen nach dem Lauf in der Übersicht,
damit die Freigabeliste bewusst erweitert werden kann.

Claude bekommt kein `GITHUB_TOKEN`. Nach jedem Lauf lädt die App `site.json` neu und stoppt vor Build,
Commit und Upload, wenn sich `build`, `output` oder `claude.allowedTools`/`disallowedTools` geändert
haben; ein Lauf kann seine eigenen Rechte so nicht erweitern. Neue Produkte anlegen ist davon nicht
betroffen.

## Upload

Hochgeladen wird von OpenSitePilot.

- Das Passwort (oder die Passphrase eines Schlüssels) liegt im Schlüsselbund des Systems, nie in
  einer Datei oder im Repository. SSH-Schlüssel werden ebenfalls unterstützt.
- Der Server-Schlüssel wird einmal bestätigt (`SHA256:…`, vergleichen mit
  `ssh-keyscan -p 22 host | ssh-keygen -lf -`); ändert er sich später, bricht jede Verbindung ab.
  „Verbindung testen“ in den Einstellungen prüft die Eingaben des Dialogs, ohne sie zu speichern; einen
  dort bestätigten Schlüssel übernimmt erst „Sichern“.
- Die App merkt sich je Ziel ein Manifest des letzten Uploads (Pfad, Größe, SHA-256) in ihrem
  Datenordner. Hochgeladen werden nur neue oder geänderte Dateien, Assets vor Seiten, jeweils unter
  einem temporären Namen und dann umbenannt, damit Besucher nie halbe Dateien bekommen. Gelöscht
  werden nur Dateien, die die App selbst hochgeladen hat; andere Dateien auf dem Server (andere
  Domains, `.htaccess`, …) bleiben unberührt.
- Hochgeladen wird immer der Ausgabeordner aus `site.json` (`"output"`, z. B. `dist` oder `build/public`).
- „Alles neu hochladen“ im Ergebnis von „Veröffentlichen“ lädt jede Datei des Builds hoch, auch
  unveränderte, etwa wenn auf dem Server Dateien fehlen oder von anderswo überschrieben wurden.
  Gelöscht wird dabei nichts anderes als beim normalen Upload.
- Hochgeladen wird nur ein committeter, sauberer Stand, und nur wenn er nicht hinter `origin` liegt.

Jede App hat ihren eigenen Datenordner, ohne Geheimnisse:

- **OpenReleasePilot**, `~/Library/Application Support/OpenReleasePilot/`: `config.json` (verwaltete
  Repositories, Inhaltsziele, Einstellungen der Zugänge), `jobs/` (je Pflegeauftrag eine Datei und ein Log;
  die 50 neuesten abgeschlossenen bleiben), `content.json` (welchen Stand jedes Inhaltsziel zuletzt
  erreicht hat) und `work/` (die Arbeitsordner offener Aufträge, entfernt, sobald ein Auftrag beendet ist).
- **OpenSitePilot**, `~/Library/Application Support/OpenSitePilot/`: `config.json` (Website- und
  SFTP-Einstellungen) und `uploads/` (die Manifeste der Uploads).

Beim ersten Start nach der Umbenennung übernimmt OpenReleasePilot den Ordner `OpenFTPUpdater-AI` und das
unter dem alten Namen gespeicherte SFTP-Passwort; macOS fragt dabei unter Umständen einmal, ob
OpenReleasePilot diesen Schlüsselbundeintrag verwenden darf.

## Bauen

```bash
cd desktop
npm ci
scripts/build-pkg.sh releasepilot   # oder: scripts/build-pkg.sh sitepilot
```

`scripts/build-pkg.sh` baut den Installer wie die CI: `dist/<App>-<version>.pkg` mit `.sha256`-Datei,
ohne Argument für OpenReleasePilot (`VERSION=0.5.0 scripts/build-pkg.sh sitepilot`). Die Tauri-CLI läuft
dabei im Ordner der App (`desktop/src-tauri` bzw. `desktop/sitepilot`), weil sich beide Apps einen
Workspace teilen. Die App liegt danach unter `desktop/target/release/bundle/macos/<App>.app` (ad-hoc
signiert). Während der Entwicklung: `cd desktop/src-tauri && npx tauri dev` bzw.
`cd desktop/sitepilot && npx tauri dev`. Tests: `cargo test --workspace`; Tests, die
GitHub lesen oder Claude ausführen (ein paar Cent), sind als `ignored` markiert und laufen mit
`cargo test -p openreleasepilot-core -- --ignored`; keiner davon schreibt in ein echtes Repository.

`SIGN_IDENTITY` (Developer ID Application) und
`INSTALLER_IDENTITY` (Developer ID Installer) signieren App und Paket; ohne sie ist die App ad-hoc
signiert und das Paket unsigniert.

`bundle.macOS.minimumSystemVersion` ist bewusst nicht gesetzt: Mit
`MACOSX_DEPLOYMENT_TARGET=14.0` erzeugte Rust 1.94 unter macOS 27 im Release-Build
Proc-Macro-Bibliotheken, die sich nicht laden ließen („mis-aligned LINKEDIT string pool“).

```
desktop/core        Rust-Bibliothek beider Apps: Einstellungen, Repositories, Pflegeregeln, Inhaltsziele,
                    Aufträge, site.json, Shell/PATH, git, GitHub, GitLab, Claude, SFTP, Manifest, Verify, Updates
desktop/src-tauri   OpenReleasePilot: Zustand, Befehle, Durchläufe der Aufträge, Symbol in der Menüleiste
desktop/sitepilot   OpenSitePilot: Zustand, Befehle, die vier Website-Schritte
desktop/ui          Web-Oberfläche beider Apps ohne Build-Schritt: index.html + releasepilot.js (OpenReleasePilot),
                    site.html + sitepilot.js (OpenSitePilot), common.js und style.css gemeinsam
```

## CI und Releases

- `.github/workflows/ci.yml`: Jeder Pull Request und jeder Push auf `main` führt `build.yml` aus (macOS 15,
  Apple Silicon): `cargo test --workspace`, `node --check` für jede `ui/*.js` und den Build beider Pakete.
- `.github/workflows/release.yml`: Ein Tag `X.Y.Z` (oder `X.Y.Z-beta.N` für ein Pre-Release) baut beide
  Apps mit dieser Version, signiert beide `.pkg` mit minisign und veröffentlicht ein GitHub-Release mit
  `OpenReleasePilot-<version>.pkg` und `OpenSitePilot-<version>.pkg`, je mit Prüfsumme und `.minisig`. Das
  Paket von OpenReleasePilot wird zuerst hochgeladen: Versionen vor der Trennung nehmen die erste `.pkg`
  eines Releases und bekommen so weiter OpenReleasePilot. Veröffentlicht wird erst nach grünen Tests und grünem Build, und nur mit dem
  Repository-Secret `MINISIGN_SECRET_KEY` (der geheime Schlüssel zu `desktop/update-key.pub`); ohne
  Secret bricht der Workflow ab, statt einen unsignierten Installer zu veröffentlichen.
- Alle Actions sind auf Commit-SHAs gepinnt (Version als Kommentar); Dependabot schlägt jeden Montag
  Updates für Actions, Crates und die Tauri-CLI vor.

```bash
git tag 0.2.0 && git push origin 0.2.0
```

## Lizenz

[MIT](LICENSE) © 2026 bl0rb
