// OpenSitePilot UI: the website's four steps, its products and update tasks, and its settings. No framework, no build step.
import {
  invoke, listen, modelName, modelSelect, h, icon, button, iconButton, menuButton, formatTime, domId, call, refresher,
  banner, updateBanner, renderToast, usd, markdown, appendTo, openDialog, row,
} from './common.js';

const STEPS = [
  { id: 'precheck', title: 'Prüfen', desc: 'Repo, Werkzeuge, Releases, SFTP und Live-Seite. Ändert nichts.' },
  { id: 'update', title: 'Inhalte aktualisieren', desc: 'Claude passt die Seite mit den gewählten Aufgaben an, dann Build und lokaler Commit.' },
  { id: 'upload', title: 'Veröffentlichen', desc: 'Build, Push, dann nur geänderte Dateien per SFTP auf den Server.' },
  { id: 'verify', title: 'Live-Seite prüfen', desc: 'Vergleicht die Live-Seite Datei für Datei mit dem Build.' },
];
const STATUS_TEXT = { idle: 'Bereit', running: 'Läuft', ok: 'OK', warning: 'Hinweis', error: 'Fehler' };

let view = null;
let selected = 'precheck';
// Tab of the detail panel: 'result' or 'log'.
let detailTab = 'result';
let lastRunning = null;
let hostKeyOpen = false;

const refresh = refresher(setView);

// MARK: Render

function setView(next) {
  view = next;
  if (view.running && view.running !== lastRunning) {
    // Follow the step that just started and watch its log.
    selected = view.running;
    detailTab = 'log';
    loadLog(selected);
  } else if (!view.running && lastRunning === selected && hasResult(selected)) {
    // The step is done: show what came out of it. Without a result the log has the details.
    detailTab = 'result';
  }
  lastRunning = view.running;
  const root = document.documentElement;
  root.classList.toggle('vibrancy', !!view.vibrancy);
  root.classList.toggle('mac', view.platform === 'macos');
  renderSidebar();
  renderBanners();
  renderMain();
  renderToast(view);
  if (view.pendingHostKey && !hostKeyOpen) hostKeyDialog();
}

let sidebarSignature = '';

/** The website on the left, settings and version below; like OpenReleasePilot. */
function renderSidebar() {
  const signature = JSON.stringify([!!view.config.projectDir, !!view.running, view.version, view.devBuild, view.checkingUpdate]);
  if (signature === sidebarSignature) return;
  sidebarSignature = signature;
  const check = (beta) => () => call('check_for_updates', { beta });
  document.getElementById('sidebar').replaceChildren(
    h('div', { class: 'brand', 'data-tauri-drag-region': '' }, h('img', { class: 'logo', src: 'site-icon.svg', alt: '' }), 'OpenSitePilot'),
    view.config.projectDir && h('button', { type: 'button', class: 'nav-item active', 'aria-current': 'page' },
      icon('globe'), h('span', null, 'Website'), view.running && h('span', { class: 'nav-badge', title: 'Ein Schritt läuft' }, '…')),
    h('div', { class: 'sidebar-footer' },
      h('button', {
        type: 'button', class: 'nav-item', disabled: !!view.running, title: 'Website-Projekt, SFTP-Ziel, Updates', onclick: settingsDialog,
      }, icon('gear'), h('span', null, 'Einstellungen')),
      h('div', { class: 'version' },
        view.devBuild ? 'OpenSitePilot Entwicklungsversion' : `OpenSitePilot ${view.version}`,
        view.checkingUpdate ? h('span', null, 'Suche nach Updates …') : button('Nach Updates suchen', check(false), { cls: 'link' }))),
  );
}

/** Title, one line of state and the website's actions: one primary action at most, frequent ones
 *  as icons, rare ones in "⋯". */
const pageHead = h('header', { class: 'page-header', 'data-tauri-drag-region': '' });
// Set by renderWebsite: { actions }.
let headerParts = null;

function renderPageHeader() {
  const title = view.site ? `Website ${view.site.name}` : 'Website';
  const subtitle = view.site ? 'Prüfen, Inhalte aktualisieren, Veröffentlichen, Live-Seite prüfen' : 'Die Projektdatei der Website ist nicht lesbar';
  const drag = { 'data-tauri-drag-region': '' };
  const actions = (headerParts?.actions ?? []).filter(Boolean);
  pageHead.replaceChildren(...[
    h('div', { class: 'page-title', ...drag },
      h('div', { class: 'page-title-line', ...drag }, h('h1', { class: 'ellipsis', ...drag }, title)),
      h('p', drag, subtitle)),
    actions.length > 0 && h('div', { class: 'page-actions' }, actions),
  ].filter(Boolean));
}

function renderBanners() {
  const items = [];
  if (view.availableUpdate) items.push(updateBanner(view, 'OpenSitePilot'));
  if (view.error) items.push(banner('error', view.error, [['Schließen', () => call('dismiss')]]));
  if (view.signedIn === false && view.site?.products?.length) {
    items.push(banner('warning', 'Die GitHub-CLI ist nicht angemeldet: Releases nur öffentlicher Repositories lesbar, mit 60 Anfragen pro Stunde. Im Terminal: gh auth login.'));
  }
  if (view.siteError) items.push(banner('warning', `Website: ${view.siteError}`, [['Einstellungen', settingsDialog]]));
  if (view.site && !view.sftpReady) items.push(banner('warning', 'SFTP ist noch nicht eingerichtet: Veröffentlichen ist erst danach möglich.', [['Einstellungen', settingsDialog]]));
  document.getElementById('banners').replaceChildren(...items);
}

let skeleton = null;

function renderMain() {
  const main = document.getElementById('main');
  if (!view.config.projectDir) {
    skeleton = null;
    main.replaceChildren(h('div', { class: 'empty' },
      h('img', { src: 'site-icon.svg', alt: '' }),
      h('h2', null, 'Willkommen'),
      h('p', { class: 'muted' }, 'OpenSitePilot hält eine statische Website aktuell: Claude Code passt die Inhalte an neue Releases deiner Produkte an, die App baut, committet, lädt per SFTP hoch und prüft die Live-Seite. Die Repositories selbst pflegt OpenReleasePilot.'),
      h('div', { class: 'line' }, button('Website einrichten …', settingsDialog, { cls: 'primary', iconName: 'plus' }))));
    return;
  }
  if (!skeleton) buildWebsite();
  renderPageHeader();
  if (main.children.length !== 2 || main.children[0] !== pageHead || main.children[1] !== skeleton.root) main.replaceChildren(pageHead, skeleton.root);
  renderWebsite();
}

// MARK: Website

function buildWebsite() {
  // Built once, so the instruction field keeps focus and the log keeps its scroll position.
  skeleton = {
    steps: h('section', { class: 'steps' }),
    products: h('div', { class: 'list products' }),
    productCount: h('span', { class: 'muted small' }),
    productAll: h('input', { type: 'checkbox', onchange: (e) => call('set_products', { ids: view.products.map((p) => p.id), enabled: e.target.checked }) }),
    tasks: h('div', { class: 'list tasks' }),
    taskTech: h('div', { class: 'checks' }),
    instructions: h('textarea', { id: 'instructions', rows: 1, placeholder: 'z. B. „Ergänze im nudgit-Abschnitt den Hinweis auf die Chrome-Erweiterung“ – leer lassen für ein reines Release-Update' }),
    detailHead: h('div', { class: 'detail-head' }),
    tabResult: h('button', { type: 'button', class: 'tab', role: 'tab', id: 'tab-result', 'aria-controls': 'detail-result', onclick: () => showTab('result') }, 'Ergebnis'),
    tabLog: h('button', { type: 'button', class: 'tab', role: 'tab', id: 'tab-log', 'aria-controls': 'log', onclick: () => showTab('log') }),
    detailResult: h('div', { class: 'detail-result', id: 'detail-result', role: 'tabpanel', 'aria-labelledby': 'tab-result' }),
    log: h('pre', { class: 'log', id: 'log', role: 'tabpanel', 'aria-labelledby': 'tab-log', tabindex: '0' }),
  };
  skeleton.root = h('div', { class: 'area' },
    h('section', { class: 'section' },
      h('div', { class: 'section-head' }, h('h2', null, 'Ablauf'),
        h('span', { class: 'muted small' }, 'Schritte einzeln starten oder oben alle nacheinander bis zur Live-Seite')),
      skeleton.steps),
    h('div', { class: 'config' },
      h('section', { class: 'section panel' },
        h('div', { class: 'section-head' }, h('h2', null, 'Produkte'), skeleton.productCount, h('span', { class: 'spacer' }),
          h('label', { class: 'check-all', title: 'Alle Produkte an- oder abwählen' }, skeleton.productAll, 'Alle')),
        skeleton.products,
        h('div', { class: 'note' }, 'Nur ausgewählte Produkte werden geprüft und aktualisiert.')),
      h('section', { class: 'section panel' },
        h('div', { class: 'section-head' }, h('h2', null, 'Update-Aufgaben'),
          h('span', { class: 'muted small' }, 'je ein Claude-Lauf, in dieser Reihenfolge')),
        skeleton.tasks,
        h('div', { class: 'field-col' }, h('label', { for: 'instructions' }, 'Zusätzliche Anweisung an Claude (optional)'), skeleton.instructions),
        h('details', { class: 'fold' }, h('summary', null, 'Technische Details der Aufgaben'),
          h('div', { class: 'note' }, 'Skill und Effort stammen aus site.json und lassen sich dort ändern.'), skeleton.taskTech))),
    h('section', { class: 'detail' }, skeleton.detailHead,
      h('div', { class: 'tabs', role: 'tablist', 'aria-label': 'Ansicht', onkeydown: tabKey }, skeleton.tabResult, skeleton.tabLog),
      skeleton.detailResult, skeleton.log));
  loadLog(selected);
}

function renderWebsite() {
  headerParts = {
    actions: view.site ? [
      iconButton('Live-Seite öffnen', () => call('open_url', { url: view.site.url }), 'external', { id: 'site-open', title: `Live-Seite öffnen: ${view.site.url}` }),
      menuButton('Weitere Aktionen', () => [
        ['Produkt hinzufügen …', addProductDialog, { iconName: 'plus', disabled: !!view.running, title: 'Ein Repository als neues Produkt auf der Website aufnehmen' }],
      ], { id: 'site-more' }),
      button('Aktualisieren und veröffentlichen', () => call('run_all', { instructions: skeleton.instructions.value }), {
        id: 'site-run-all', cls: 'primary', iconName: 'forward', disabled: !!view.running,
        title: 'Prüfen, Inhalte aktualisieren (wenn nötig), Veröffentlichen und Live-Seite prüfen nacheinander; geht bis zur Live-Seite und stoppt beim ersten Fehler',
      }),
    ] : [],
  };
  renderPageHeader();
  skeleton.instructions.disabled = !!view.running;
  renderSteps();
  renderProducts();
  renderTasks();
  renderDetail();
}


function versionBadge(p) {
  const latest = p.latestTag?.replace(/^v/, '');
  if (!latest) return h('span', { class: 'badge', title: 'Dokumentierter Stand; „Prüfen“ fragt das neueste Release ab' }, p.documented);
  if (p.outdated) return h('span', { class: 'badge orange', title: `Neues Release ${p.latestTag}` }, `${p.documented} → ${latest}`);
  return h('span', { class: 'badge green', title: 'Dokumentierter Stand ist das neueste Release' }, `${p.documented} aktuell`);
}

function renderProducts() {
  const products = view.products;
  const on = products.filter((p) => p.enabled).length;
  skeleton.productCount.textContent = products.length ? `${on} von ${products.length} ausgewählt` : '';
  skeleton.productAll.checked = products.length > 0 && on === products.length;
  skeleton.productAll.indeterminate = on > 0 && on < products.length;
  skeleton.productAll.disabled = !!view.running || !products.length;
  skeleton.products.replaceChildren(...(products.length ? [
    h('div', { class: 'row head' }, h('span', null, 'Produkt · Repository'), h('span'), h('span', null, 'Stand')),
    ...products.map((p) => h('div', { class: `row ${p.enabled ? 'on' : ''}` },
    h('label', { class: 'row-name' },
      h('input', { type: 'checkbox', checked: p.enabled, disabled: !!view.running, onchange: (e) => call('set_products', { ids: [p.id], enabled: e.target.checked }) }),
      h('span', { class: 'row-text inline' }, h('strong', null, p.name), h('span', { class: 'muted small mono ellipsis', title: p.repo }, p.repo))),
    p.local ? h('span') : h('span', { class: 'badge orange', title: p.localPath ? `Kein lokaler Klon unter ${p.localPath}` : 'Kein localPath in site.json' }, 'kein Klon'),
    versionBadge(p)))]
    : [h('div', { class: 'note empty-list' }, 'Noch keine Produkte: „Produkt hinzufügen …“ im ⋯-Menü oben rechts nimmt eins auf.')]));
}

function renderTasks() {
  const head = h('div', { class: 'row head' }, h('span', null, 'Aufgabe'), h('span', null, 'Modell'));
  skeleton.taskTech.replaceChildren(...view.tasks.map((t) => [h('span'), h('span', { class: 'label' }, t.label),
    h('span', { class: 'detail-text' }, [t.skill ? `Skill ${t.skill}` : 'ohne Skill', t.effort && `Effort ${t.effort}`].filter(Boolean).join(' · '))]));
  skeleton.tasks.replaceChildren(head, ...view.tasks.map((t) => {
    const save = (enabled, model) => call('set_task', { id: t.id, enabled, model });
    // One row per task; the columns line up across rows (CSS subgrid).
    return h('div', { class: `row ${t.enabled ? 'on' : ''}` },
      h('label', { class: 'row-name' },
        h('input', { type: 'checkbox', checked: t.enabled, disabled: !!view.running, onchange: (e) => save(e.target.checked, t.chosenModel) }),
        h('span', { class: 'row-text' }, h('strong', null, t.label), t.description && h('span', { class: 'muted small' }, t.description))),
      modelSelect(t.chosenModel, t.defaultModel, (model) => save(t.enabled, model), `task-model-${t.id}`));
  }));
}

function ready(step) {
  if (!view.site) return step === 'precheck' && !!view.config.projectDir;
  if (step === 'upload') return view.sftpReady;
  return true;
}

/** The step that makes sense next, highlighted as the primary button. */
function suggested() {
  const s = view.steps;
  if (view.running) return null;
  if (s.precheck.status === 'idle') return 'precheck';
  if (view.outdated?.length) return 'update';
  if (s.update.status === 'ok' && s.upload.status !== 'ok') return 'upload';
  if (s.upload.status === 'ok' && s.verify.status === 'idle') return 'verify';
  return null;
}

function statusChip(status, stale = false) {
  if (stale) return h('span', { class: 'chip idle', title: 'Stand vor der letzten Aktualisierung oder Veröffentlichung' }, 'Veraltet');
  return h('span', { class: `chip ${status}` }, status === 'running' && h('span', { class: 'spinner' }), STATUS_TEXT[status]);
}

function renderSteps() {
  const next = suggested();
  skeleton.steps.replaceChildren(...STEPS.map((step, i) => {
    const state = view.steps[step.id];
    const running = view.running === step.id;
    const action = running
      ? button('Abbrechen', () => invoke('cancel'), { iconName: 'stop', cls: 'danger' })
      : button('Starten', () => call('run_step', { step: step.id, instructions: skeleton.instructions.value }),
        // The page's primary action is "Aktualisieren und veröffentlichen"; the suggested step stands out more quietly.
        { iconName: 'play', cls: next === step.id ? 'suggested' : null, disabled: !!view.running || !ready(step.id) });
    const summary = state.stale ? 'Nach Aktualisierung oder Veröffentlichung erneut prüfen' : state.summary;
    return h('div', {
      class: `step ${selected === step.id ? 'selected' : ''}`, role: 'button', tabindex: '0',
      onclick: () => selectStep(step.id), onkeydown: (e) => { if (e.key === 'Enter') selectStep(step.id); },
    },
      h('div', { class: 'step-head' }, h('span', { class: 'step-number' }, i + 1), h('h2', null, step.title), statusChip(state.status, state.stale)),
      h('div', { class: 'desc' }, step.desc),
      h('div', { class: `summary ${state.stale ? 'muted' : state.status === 'error' ? 'red' : state.status === 'warning' ? 'orange' : 'muted'}`, title: summary || null },
        summary || ''),
      action);
  }));
}

function selectStep(step) {
  if (selected === step) return;
  selected = step;
  if (view.running === step) detailTab = 'log';
  renderSteps();
  renderDetail();
  loadLog(step);
}

const RESULTS = { precheck: precheckResult, update: updateResult, upload: uploadResult, verify: verifyResult };
const HAS_RESULT = {
  precheck: () => view.checks.length > 0,
  update: () => view.claudeRuns.length > 0,
  upload: () => !!view.upload,
  verify: () => !!view.verify,
};
const hasResult = (step) => HAS_RESULT[step]?.() ?? false;

function renderDetail() {
  const step = STEPS.find((s) => s.id === selected);
  const state = view.steps[selected];
  skeleton.detailHead.replaceChildren(...[
    h('h3', null, step.title), statusChip(state.status, state.stale),
    state.summary && !state.stale && h('span', { class: `detail-summary ${state.status === 'error' ? 'red' : state.status === 'warning' ? 'orange' : ''}` }, state.summary),
    h('span', { class: 'spacer' }),
    h('span', { class: 'muted small' }, state.finishedAt ? `zuletzt ${formatTime(state.finishedAt)}` : ''),
  ].filter(Boolean));
  const empty = !hasResult(selected) && (state.status === 'idle'
    ? h('div', { class: 'note' }, 'Dieser Schritt ist noch nicht gelaufen.')
    : h('div', { class: 'note' }, view.running === selected ? 'Läuft gerade. Die Ausgabe steht im ' : 'Kein Ergebnis. Was passiert ist, steht im ',
      button('Log', () => showTab('log'), { cls: 'link' }), '.'));
  skeleton.detailResult.replaceChildren(...[...RESULTS[selected](), empty].filter(Boolean));
  renderTabs();
}

function renderTabs() {
  const log = detailTab === 'log';
  const showsLog = !skeleton.log.hidden;
  for (const [tab, on] of [[skeleton.tabResult, !log], [skeleton.tabLog, log]]) {
    tab.setAttribute('aria-selected', String(on));
    tab.tabIndex = on ? 0 : -1;
  }
  skeleton.tabLog.replaceChildren(view.running === selected ? h('span', { class: 'spinner' }) : '', 'Log');
  skeleton.detailResult.hidden = log;
  skeleton.log.hidden = !log;
  // A hidden log loses its scroll position; show the newest lines again.
  if (log && !showsLog) skeleton.log.scrollTop = skeleton.log.scrollHeight;
}

function showTab(tab) {
  if (detailTab === tab) return;
  detailTab = tab;
  renderTabs();
}

/** Arrow keys move between the two tabs, as in any tab bar. */
function tabKey(e) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
  e.preventDefault();
  const next = e.key === 'Home' ? 'result' : e.key === 'End' ? 'log' : detailTab === 'log' ? 'result' : 'log';
  showTab(next);
  (next === 'log' ? skeleton.tabLog : skeleton.tabResult).focus();
}

function precheckResult() {
  if (!view.checks.length) return [];
  const stale = view.steps.precheck.stale;
  const unknown = view.releaseUnknown.length > 0 && !stale
    && h('div', { class: 'note orange' }, `Release-Stand unklar: ${view.releaseUnknown.join('; ')}. „Inhalte aktualisieren“ und „Aktualisieren und veröffentlichen“ stoppen, bis der Stand bekannt ist.`);
  const outdated = view.outdated?.length
    ? h('div', { class: 'facts' }, h('span', null, h('strong', null, 'Neue Releases: '),
      view.outdated.map((o) => `${o.name} ${o.documented} → ${o.latestTag.replace(/^v/, '')}`).join(', ')))
    : null;
  return [
    stale && h('div', { class: 'note orange' }, 'Dieses Ergebnis stammt von vor der letzten Aktualisierung oder Veröffentlichung (z. B. Remote-Stand und Releases). „Prüfen“ erneut starten für den aktuellen Stand.'),
    outdated && !stale ? outdated : null,
    unknown,
    h('div', { class: `checks ${stale ? 'stale' : ''}` }, view.checks.map((c) => [
    h('span', { class: `status-dot ${c.status}` }), h('span', { class: 'label' }, c.label), h('span', { class: 'detail-text selectable' }, c.detail),
  ]))];
}

// Which Claude runs the user opened or closed; survives re-rendering.
const runOpen = new Map();

function runItem(run, index, count) {
  const c = run.outcome;
  const key = `${index}:${run.task}`;
  const open = runOpen.get(key) ?? (count === 1 || !c.success || c.denials.length > 0);
  return h('details', { class: 'run', open, ontoggle: (e) => runOpen.set(key, e.target.open) },
    h('summary', null,
      h('strong', null, run.task),
      h('span', { class: 'muted small' }, [modelName(run.model === 'Standard' ? null : run.model),
        c.turns != null && `${c.turns} Runden`, c.costUsd != null && usd(c.costUsd)].filter(Boolean).join(' · ')),
      c.denials.length > 0 && h('span', { class: 'badge orange' }, `${c.denials.length} nicht freigegeben`),
      h('span', { class: 'spacer' }),
      h('span', { class: `chip ${c.success ? 'ok' : 'error'}` }, c.success ? 'Abgeschlossen' : 'Nicht abgeschlossen')),
    h('div', { class: 'run-body' },
      c.denials.length > 0 && h('div', { class: 'denials' },
        h('div', null, h('strong', null, 'Nicht freigegeben: '),
          'Claude durfte diese Befehle nicht ausführen und hat ohne sie weitergearbeitet. Dauerhaft erlauben lässt sich ein Befehl über ',
          h('code', null, 'claude.allowedTools'), ' in ', h('code', null, view.siteFile), '.'),
        h('ul', null, c.denials.map((d) => h('li', { class: 'mono selectable', title: d }, h('span', { class: 'clamp' }, d))))),
      c.text ? markdown(c.text) : h('div', { class: 'muted' }, 'Keine Antwort von Claude.')));
}

function updateResult() {
  const runs = view.claudeRuns;
  if (!runs.length) runOpen.clear();
  const items = [h('div', { class: 'facts' },
    h('span', null, h('strong', null, 'Gewählte Aufgaben: '), view.tasks.filter((t) => t.enabled).map((t) => t.label).join(', ') || 'keine'),
    view.site && h('span', null, h('strong', null, 'Build: '), h('span', { class: 'mono' }, view.site.build)))];
  if (runs.length) {
    const cost = runs.reduce((sum, r) => sum + (r.outcome.costUsd ?? 0), 0);
    const turns = runs.reduce((sum, r) => sum + (r.outcome.turns ?? 0), 0);
    items.push(h('div', { class: 'runs' },
      h('div', { class: 'runs-head' }, h('strong', null, runs.length === 1 ? '1 Claude-Lauf' : `${runs.length} Claude-Läufe`),
        h('span', { class: 'muted small' }, `${turns} Runden · ${usd(cost)}`)),
      runs.map((run, i) => runItem(run, i, runs.length))));
  }
  return items;
}

function uploadResult() {
  const items = [];
  const cfg = view.config.sftp;
  items.push(h('div', { class: 'facts' },
    view.site && h('span', null, h('strong', null, 'Quelle: '), `${view.config.projectDir}/${view.site.output}`,
      h('span', { class: 'muted small' }, ` (output in ${view.siteFile})`)),
    h('span', null, h('strong', null, 'Ziel: '), cfg.host ? `${cfg.user}@${cfg.host}:${cfg.port} ${cfg.remoteDir}` : 'nicht eingerichtet'),
    view.lastUpload && h('span', null, h('strong', null, 'Letzter Upload: '),
      `${view.lastUpload.uploadedAt ? formatTime(view.lastUpload.uploadedAt) : '?'} · Commit ${view.lastUpload.commit ?? '?'} · ${view.lastUpload.files} Dateien`)));
  if (view.upload) {
    items.push(h('div', { class: 'facts' },
      h('span', null, h('strong', null, 'Hochgeladen: '), view.upload.uploaded),
      h('span', null, h('strong', null, 'Entfernt: '), view.upload.deleted),
      h('span', null, h('strong', null, 'Unverändert: '), view.upload.unchanged),
      h('span', null, h('strong', null, 'Git: '), view.upload.pushed)));
  }
  // The step button sends only changes; this sends every file again, e.g. after the server lost some.
  items.push(h('div', { class: 'facts' },
    button('Alles neu hochladen', () => call('run_step', { step: 'upload', instructions: '', everything: true }),
      { iconName: 'upload', disabled: !!view.running || !ready('upload'),
        title: 'Build, Push, dann jede Datei des Ausgabeordners per SFTP hochladen, auch unveränderte' })));
  return items;
}

function verifyResult() {
  const r = view.verify;
  if (!r) return [];
  return [
    h('div', { class: 'facts' },
      h('span', null, h('strong', null, 'Geprüft: '), r.checked),
      h('span', { class: 'green' }, h('strong', null, 'Identisch: '), r.identical),
      r.problems.length > 0 && h('span', { class: 'red' }, h('strong', null, 'Abweichend: '), r.problems.length)),
    r.problems.length > 0 && h('div', { class: 'result-text mono' }, r.problems.slice(0, 50).join('\n') + (r.problems.length > 50 ? `\n… ${r.problems.length - 50} weitere` : '')),
  ];
}


// MARK: Log

function clearLog() {
  if (skeleton) skeleton.log.replaceChildren();
}

function appendLog(lines) {
  if (skeleton) appendTo(skeleton.log, lines);
}

async function loadLog(step) {
  const lines = await invoke('get_log', { step });
  if (step !== selected || !skeleton) return;
  clearLog();
  appendLog(lines);
}


// MARK: Repository picker

let repoCache = null;

async function loadRepos(force = false) {
  if (!repoCache || force) repoCache = await invoke('list_repos');
  return repoCache;
}

/** Filterable list of the user's repositories. Keeps its own state, so typing never re-renders the
 *  dialog. With `multi`, rows toggle in `picked` (a Set of full names); else `onPick` gets the row. */
function repoPicker({ include = () => true, onPick, multi = false, picked = new Set(), locked = () => null }) {
  const state = { query: '', data: null, error: null, picked: null };
  const list = h('div', { class: 'repo-list', role: 'listbox', 'aria-multiselectable': multi ? 'true' : null });
  const note = h('div', { class: 'note' });
  const search = h('input', { type: 'search', id: 'pick-search', placeholder: 'Repositories filtern …', spellcheck: 'false', autocomplete: 'off', 'aria-label': 'Repositories filtern',
    oninput: (e) => { state.query = e.target.value.trim().toLowerCase(); renderList(); } });
  const tag = (text, cls = '') => h('span', { class: `badge ${cls}` }, text);
  const renderList = () => {
    const focusId = list.contains(document.activeElement) ? document.activeElement.id : null;
    renderRows();
    if (focusId) document.getElementById(focusId)?.focus({ preventScroll: true });
  };
  const renderRows = () => {
    if (state.error) { list.replaceChildren(h('div', { class: 'note error-text' }, state.error)); return; }
    if (!state.data) { list.replaceChildren(h('div', { class: 'note' }, 'Repositories werden geladen …')); return; }
    const repos = state.data.repos.filter(include).filter((r) => !state.query
      || r.fullName.toLowerCase().includes(state.query) || (r.description ?? '').toLowerCase().includes(state.query));
    note.textContent = [state.data.note, `Lokale Klone werden in ${state.data.workspace} gesucht.`].filter(Boolean).join(' ');
    list.replaceChildren(...(repos.length ? repos.map((r) => {
      const why = locked(r);
      const on = multi ? picked.has(r.fullName) : state.picked === r.fullName;
      return h('button', {
        type: 'button', role: 'option', id: domId('pick-row', r.fullName), 'aria-selected': String(on), class: `repo-row ${on ? 'picked' : ''}`, disabled: !!why, title: why,
        onclick: () => {
          if (multi) on ? picked.delete(r.fullName) : picked.add(r.fullName);
          else state.picked = r.fullName;
          renderList();
          onPick?.(r);
        },
      },
        h('div', { class: 'repo-head' },
          multi && h('input', { type: 'checkbox', checked: on, disabled: !!why, tabindex: '-1', 'aria-hidden': 'true' }),
          h('strong', null, r.fullName),
          r.private && tag('privat', 'orange'),
          r.isWebsite && tag('Website', 'accent'),
          r.onSite && tag('Produkt der Website', ''),
          r.archived && tag('archiviert', 'orange'),
          r.localPath ? tag('lokal', 'green') : tag('nicht lokal')),
        r.description && h('div', { class: 'muted small' }, r.description));
    }) : [h('div', { class: 'note' }, 'Keine passenden Repositories.')]));
  };
  const load = async (force) => {
    state.error = null;
    state.data = force ? null : state.data;
    renderList();
    try { state.data = await loadRepos(force); } catch (e) { state.error = String(e); }
    renderList();
  };
  const el = h('div', { class: 'repo-picker' },
    h('div', { class: 'line' }, search, button('Neu laden', () => load(true), { id: 'pick-reload', iconName: 'refresh', iconOnly: true })),
    list, note);
  load(false);
  return el;
}

// MARK: Settings

/** The cost limit as typed, as the app checks it: empty = none, else a positive amount with point or comma. */
function settingsDialog() {
  const c = view.config;
  const s = {
    projectDir: c.projectDir, workspaceDir: c.workspaceDir ?? '', siteRepo: view.siteRepo, cloneRepo: null, cloning: false,
    showPicker: false,
    host: c.sftp.host, port: c.sftp.port || 22, user: c.sftp.user, remoteDir: c.sftp.remoteDir,
    auth: c.sftp.auth, keyPath: c.sftp.keyPath, secret: '', message: null, busy: false,
    updateCheck: !c.noUpdateCheck,
    // The connection test's answer, shown at the test; a server key it offered, and the one trusted here (saved with the rest).
    test: null, offeredKey: null, hostKey: '', hostKeyFor: '',
    open: { website: true, app: false },
  };
  const target = () => `${s.host.trim()}:${Number(s.port) || 22}`;
  const input = () => ({
    projectDir: s.projectDir, workspaceDir: s.workspaceDir, host: s.host, port: Number(s.port) || 22, user: s.user, remoteDir: s.remoteDir,
    auth: s.auth, keyPath: s.keyPath, secret: s.secret, updateCheck: s.updateCheck,
    hostKey: s.hostKey && s.hostKeyFor === target() ? s.hostKey : '',
  });
  const text = (key, props = {}, after) => h('input', { type: 'text', value: s[key] ?? '', spellcheck: 'false', autocomplete: 'off', ...props,
    oninput: (e) => { s[key] = e.target.value; after?.(); } });
  let render = () => {};
  const picker = repoPicker({
    include: (r) => !r.archived,
    onPick: (r) => {
      s.siteRepo = r.fullName;
      s.cloneRepo = r.localPath ? null : r.fullName;
      if (r.localPath) { s.projectDir = r.localPath; s.showPicker = false; }
      render();
    },
  });
  const section = (key, title, ...content) => h('details', { class: 'fold settings-group', open: s.open[key], ontoggle: (e) => { s.open[key] = e.target.open; } },
    h('summary', null, title), h('div', { class: 'fold-body' }, content));
  const testConnection = async () => {
    s.busy = true; s.test = { cls: 'note', text: 'Verbinde …' }; s.offeredKey = null; render();
    try {
      const result = await invoke('test_connection', { input: input() });
      if (result.kind === 'unknownHostKey') {
        s.offeredKey = result.fingerprint;
        s.test = { cls: 'orange', text: 'Der Server ist noch nicht bestätigt. Schlüssel prüfen, dann vertrauen und erneut testen.' };
      } else {
        s.test = { cls: 'ok-text', text: `${result.message}. Noch nicht gesichert: erst „Sichern“ übernimmt die Einstellungen.` };
      }
    } catch (error) {
      s.test = { cls: 'error-text', text: String(error) };
    }
    s.busy = false; render();
  };
  const dlg = openDialog('Einstellungen', (close, rerender) => {
    render = rerender;
    const password = s.auth === 'password';
    const sameProject = c.projectDir === s.projectDir;
    const savedKey = c.sftp.hostKey && `${c.sftp.host.trim()}:${c.sftp.port || 22}` === target() ? c.sftp.hostKey : '';
    const trusted = s.hostKey && s.hostKeyFor === target() ? s.hostKey : '';
    return {
      body: [
        h('h2', null, 'Einstellungen'),
        section('website', 'Website und Veröffentlichung',
          h('fieldset', null, h('legend', null, 'Website'),
            row('Website-Repo', h('div', { class: 'line' },
              h('span', { class: `spacer ${s.siteRepo ? 'mono' : 'muted'}` }, s.siteRepo ?? 'nicht gewählt'),
              button(s.showPicker ? 'Liste schließen' : 'Aus Liste wählen …', () => { s.showPicker = !s.showPicker; render(); }, { id: 's-site-pick' }))),
            s.showPicker && picker,
            s.cloneRepo && h('div', { class: 'line' },
              h('span', { class: 'note spacer' }, `${s.cloneRepo} ist noch nicht lokal vorhanden.`),
              button(s.cloning ? 'Klone …' : `Nach ${s.workspaceDir || view.workspaceDisplay} klonen`, async () => {
                s.cloning = true; render();
                try {
                  s.projectDir = await invoke('clone_repo', { repo: s.cloneRepo, workspaceDir: s.workspaceDir });
                  s.cloneRepo = null; s.showPicker = false; repoCache = null;
                } catch (error) { s.message = String(error); }
                s.cloning = false; render();
              }, { id: 's-clone', cls: 'primary', disabled: s.cloning })),
            row('Projektordner', h('div', { class: 'line' }, text('projectDir', { id: 's-project', placeholder: 'leer = keine Website' }),
              button('Auswählen …', async () => { const p = await call('pick_folder'); if (p) { s.projectDir = p; render(); } }, { id: 's-project-pick' }),
              s.projectDir && button('Entfernen', () => { s.projectDir = ''; s.siteRepo = null; render(); }, { id: 's-project-remove', title: 'Website aus der App nehmen; im Projekt ändert sich nichts' }))),
            row('Repo-Ordner', h('div', { class: 'line' }, text('workspaceDir', { id: 's-workspace', placeholder: `${view.workspaceDisplay} (automatisch)` }),
              button('Auswählen …', async () => { const p = await call('pick_folder'); if (p) { s.workspaceDir = p; repoCache = null; render(); } }, { id: 's-workspace-pick' }))),
            h('div', { class: `note ${view.siteError && sameProject ? 'error-text' : ''}` },
              !s.projectDir ? 'Keine Website eingerichtet.'
                : view.site && sameProject ? `${view.siteFile} gefunden: ${view.site.name}, Build „${view.site.build}“, Ausgabe ${view.site.output}`
                  : view.siteError && sameProject ? view.siteError
                    : 'Der Projektordner braucht eine Datei .openreleasepilot/site.json (siehe README); geprüft wird beim Sichern.')),
          s.projectDir && h('fieldset', null, h('legend', null, 'SFTP-Ziel der Website'),
            row('Server', text('host', { id: 's-host', placeholder: 'ssh.strato.de' })),
            row('Port', text('port', { id: 's-port', inputmode: 'numeric' })),
            row('Benutzer', text('user', { id: 's-user', placeholder: 'z. B. example.com' })),
            row('Zielordner', text('remoteDir', { id: 's-remote', placeholder: '/example.com' })),
            h('div', { class: 'note' }, 'Ordner auf dem Server, der dem Ausgabeordner entspricht. Gelöscht werden nur Dateien, die die App selbst hochgeladen hat.'),
            row('Anmeldung', h('select', { onchange: (e) => { s.auth = e.target.value; render(); } },
              h('option', { value: 'password', selected: password }, 'Passwort'),
              h('option', { value: 'key', selected: !password }, 'SSH-Schlüssel')), 's-auth'),
            !password && row('Schlüsseldatei', h('div', { class: 'line' }, text('keyPath', { id: 's-key', placeholder: '~/.ssh/id_ed25519' }),
              button('Auswählen …', async () => { const p = await call('pick_file'); if (p) { s.keyPath = p; render(); } }, { id: 's-key-pick' }))),
            row(password ? 'Passwort' : 'Passphrase', h('input', { type: 'password', value: s.secret, autocomplete: 'off',
              placeholder: c.sftp.secretSaved ? 'gespeichert – leer lassen zum Behalten' : (password ? '' : 'leer, wenn der Schlüssel keine hat'),
              oninput: (e) => { s.secret = e.target.value; } }), 's-secret'),
            h('div', { class: 'note' }, 'Das Passwort liegt nur im Schlüsselbund des Systems, nie in einer Datei oder im Repository.',
              c.sftp.secretSaved && [' ', button('Gespeichertes Passwort entfernen', async () => { await call('forget_secret'); close(); }, { id: 's-forget', cls: 'link' })]),
            row('Server-Schlüssel', h('div', { class: trusted || savedKey ? 'mono selectable' : 'muted' },
              trusted ? `${trusted} (hier bestätigt; wird mit „Sichern“ gespeichert)` : savedKey || 'wird beim ersten Verbindungstest bestätigt')),
            h('div', { class: 'line wrap' },
              button(s.busy ? 'Verbinde …' : 'Verbindung testen', testConnection, { id: 's-test', disabled: s.busy || s.cloning, title: 'Testet die Eingaben dieses Dialogs; gesichert wird erst mit „Sichern“' }),
              h('span', { class: 'note' }, 'Speichert nichts.')),
            h('div', { class: `note ${s.test?.cls ?? ''}`, role: 'status', 'aria-live': 'polite' }, s.test?.text ?? ''),
            s.offeredKey && h('div', { class: 'note-box warning' },
              h('strong', null, `Server-Schlüssel von ${target()}`),
              h('div', { class: 'fingerprint' }, s.offeredKey),
              h('div', { class: 'note' }, 'Vergleiche ihn mit der Angabe deines Hosters oder im Terminal mit:'),
              h('div', { class: 'fingerprint' }, `ssh-keyscan -p ${Number(s.port) || 22} ${s.host.trim()} | ssh-keygen -lf -`),
              h('div', { class: 'line' },
                button('Vertrauen und erneut testen', () => { s.hostKey = s.offeredKey; s.hostKeyFor = target(); s.offeredKey = null; testConnection(); }, { id: 's-trust', cls: 'primary' }),
                button('Nicht vertrauen', () => { s.offeredKey = null; s.test = null; render(); }, { id: 's-distrust' }))))),
        section('app', 'App',
          h('label', { class: 'check-line' },
            h('input', { type: 'checkbox', id: 's-update-check', checked: s.updateCheck, onchange: (e) => { s.updateCheck = e.target.checked; } }),
            'Beim Start nach neuen Versionen suchen'),
          h('div', { class: 'note' }, 'Fragt das neueste Release auf GitHub ab. Installiert wird nur auf Klick, nach Prüfung von SHA-256-Prüfsumme und Signatur.'),
          h('div', { class: 'line' },
            button(view.checkingUpdate ? 'Suche läuft …' : 'Jetzt nach Beta-Version suchen', () => call('check_for_updates', { beta: true }),
              { id: 's-beta', cls: 'link', disabled: view.checkingUpdate, title: 'Beta-Versionen sind zum Testen gedacht; gefunden wird auch eine neuere stabile Version' }))),
        s.message && h('div', { class: 'note error-text', role: 'alert', id: 's-message', tabindex: '-1' }, s.message),
      ],
      actions: [
        button('Abbrechen', close),
        button('Sichern', async () => {
          try {
            await invoke('save_settings', { input: input() });
            close();
          } catch (error) {
            s.message = String(error); render();
            document.getElementById('s-message')?.focus();
          }
        }, { id: 's-save', cls: 'primary', disabled: s.busy || s.cloning }),
      ],
    };
  });
  return dlg;
}

// MARK: New product

function addProductDialog() {
  const s = { repo: null, check: null, checking: false, error: null, name: '', localPath: '', instructions: '', cloning: false, model: '' };
  let render = () => {};
  const inspect = async (repo) => {
    s.repo = repo; s.check = null; s.error = null; s.checking = true;
    s.name = repo.name; s.localPath = repo.localPath ?? '';
    render();
    try {
      s.check = await invoke('inspect_repo', { repo: repo.fullName });
      if (s.check.localPath) s.localPath = s.check.localPath;
    } catch (error) { s.error = String(error); }
    s.checking = false; render();
  };
  const picker = repoPicker({ include: (r) => !r.onSite && !r.isWebsite && !r.archived && !r.fork, onPick: inspect });
  openDialog('Produkt zur Website hinzufügen', (close, rerender) => {
    render = rerender;
    const info = s.check?.info;
    return {
      body: [
        h('h2', null, 'Produkt zur Website hinzufügen'),
        h('p', { class: 'note', style: 'margin:0' }, 'Wähle das Repository des Produkts. Claude legt Produktseite (Deutsch und Englisch), Produktkarte, Navigation, Icon, Ablaufgrafik, llms.txt und den Eintrag in site.json an, nach dem Skill des Projekts. Danach baut die App und committet lokal; live geht es erst mit „Veröffentlichen“.'),
        picker,
        s.checking && h('div', { class: 'note' }, `${s.repo.fullName} wird geprüft …`),
        s.error && h('div', { class: 'note error-text', role: 'alert' }, s.error),
        info && h('fieldset', null, h('legend', null, info.fullName),
          h('div', { class: 'facts' },
            h('span', null, h('strong', null, 'Release: '), s.check.latestTag ?? 'noch keins'),
            h('span', null, h('strong', null, 'Lizenz: '), info.license ?? 'keine erkannt'),
            info.private && h('span', { class: 'orange' }, h('strong', null, 'privat: '), 'Besucher können GitHub-Links darauf nicht öffnen')),
          info.description && h('div', { class: 'muted' }, info.description),
          !s.check.latestTag && h('div', { class: 'note orange' }, 'Ohne Release gibt es kein Versions-Badge, und „Prüfen“ beobachtet das Produkt erst nach dem ersten Release.'),
          row('Name', h('input', { type: 'text', value: s.name, spellcheck: 'false', oninput: (e) => { s.name = e.target.value; } }), 'n-name'),
          row('Lokaler Klon', h('div', { class: 'line' },
            h('span', { class: `spacer ${s.localPath ? 'mono' : 'muted'}` }, s.localPath || 'nicht vorhanden – Claude liest dann nur über GitHub'),
            !s.localPath && button(s.cloning ? 'Klone …' : 'Klonen', async () => {
              s.cloning = true; render();
              try { s.localPath = await invoke('clone_repo', { repo: info.fullName, workspaceDir: '' }); repoCache = null; } catch (error) { s.error = String(error); }
              s.cloning = false; render();
            }, { id: 'n-clone', disabled: s.cloning }),
            button('Auswählen …', async () => { const p = await call('pick_folder'); if (p) { s.localPath = p; render(); } }, { id: 'n-local-pick' })), 'n-local'),
          row('Modell', modelSelect(s.model, view.site?.claude?.model, (model) => { s.model = model; }, 'n-model'), 'n-model'),
          h('div', { class: 'field-col' },
            h('label', { for: 'n-instructions' }, 'Anweisung an Claude (optional)'),
            h('textarea', { id: 'n-instructions', rows: 3, value: s.instructions,
              placeholder: 'z. B. Kategorie, Zielgruppe, Position in der Produktliste, welche Screenshots aus dem Repo',
              oninput: (e) => { s.instructions = e.target.value; } }))),
      ],
      actions: [
        button('Abbrechen', close),
        button('Produktseite erstellen', async () => {
          // A command without a return value resolves to null, so success is "no exception".
          try {
            await invoke('run_add_product', { input: { repo: info.fullName, name: s.name, localPath: s.localPath, instructions: s.instructions, model: s.model } });
            close();
          } catch (error) { s.error = String(error); render(); }
        }, { id: 'n-create', cls: 'primary', disabled: !info || s.checking || s.cloning || !s.name.trim() || !!view.running,
          title: 'Startet einen Claude-Lauf (kostet Claude-Nutzung), danach Build und lokaler Commit' }),
      ],
    };
  });
}

function hostKeyDialog() {
  hostKeyOpen = true;
  const cfg = view.config.sftp;
  const fingerprint = view.pendingHostKey;
  const { dialog } = openDialog('Server-Schlüssel bestätigen', (close) => ({
    body: [
      h('h2', null, 'Server-Schlüssel bestätigen'),
      h('p', { class: 'muted', style: 'margin:0' }, `Erste Verbindung zu ${cfg.host}:${cfg.port}. Der Server weist sich mit diesem Schlüssel aus:`),
      h('div', { class: 'fingerprint' }, fingerprint),
      h('p', { class: 'note', style: 'margin:0' }, 'Vergleiche ihn mit der Angabe deines Hosters oder im Terminal mit:'),
      h('div', { class: 'fingerprint' }, `ssh-keyscan -p ${cfg.port} ${cfg.host} | ssh-keygen -lf -`),
      h('p', { class: 'note', style: 'margin:0' }, 'Nach dem Bestätigen bricht jede Verbindung ab, falls sich der Schlüssel später ändert.'),
    ],
    actions: [
      button('Abbrechen', async () => { await call('reject_host_key'); close(); }),
      button('Vertrauen', async () => { await call('trust_host_key', { fingerprint }); close(); }, { cls: 'primary' }),
    ],
  }));
  dialog.addEventListener('close', () => { hostKeyOpen = false; });
}

// MARK: Start

async function start() {
  await listen('changed', refresh);
  await listen('log', (event) => {
    const { step, line } = event.payload;
    if (step === selected) appendLog([line]);
  });
  // A hidden window gets no events; shown again, it fetches what it missed.
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible' || !view) return;
    refresh();
    if (selected && skeleton) loadLog(selected);
  });
  await refresh();
}

start();
