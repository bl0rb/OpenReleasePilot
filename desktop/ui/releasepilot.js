// OpenReleasePilot UI: renders the view the Rust side sends and calls its commands. No framework, no build step.
import {
  invoke, listen, modelName, modelSelect, h, icon, button, iconButton, menuButton, formatTime, ago, short, domId, call, act, refresher,
  banner, updateBanner, renderToast, usd, markdown, appendTo, openDialog, confirmDialog, row,
} from './common.js';


let view = null;
// Main area: 'repos' (start) or 'runs' (care jobs, shown as "Aufträge"). The website is OpenSitePilot's.
let area = 'repos';
// Repository whose details are open; null = the list. The list's scroll position for the way back.
let repoKey = null;
let repoListScroll = 0;
// Job whose details are open in "Aufträge"; its tab; the list's scroll position.
let jobId = null;
let jobTab = 'result';
let jobListScroll = 0;
// Repositories chosen for the next job.
const selection = new Set();
let reposRead = false;
let lastJobRunning = null;

const repoName = (key) => view.repositories.find((r) => r.key === key)?.repo ?? key;
/** The bump a job gives one repository: its own, else the job's. */
const bumpOf = (job, key) => job.options.bumps?.[key] ?? job.options.bump ?? 'patch';
/** The managed repositories whose release `key` publishes (their "Veröffentlicht durch"). */
const releasedThrough = (key) => view.repositories.filter((r) => r.releasedBy === key);

const refresh = refresher(setView);

// MARK: Render

const JOB_FINISHED = new Set(['succeeded', 'partial', 'failed', 'cancelled']);
const openJobs = () => view.jobs.filter((j) => !JOB_FINISHED.has(j.status.state));

function setView(next) {
  view = next;
  if (lastJobRunning && !view.jobRunning && lastJobRunning === jobId) jobTab = 'result';
  lastJobRunning = view.jobRunning;
  for (const key of [...selection]) if (!view.repositories.some((r) => r.key === key)) selection.delete(key);
  const root = document.documentElement;
  root.classList.toggle('vibrancy', !!view.vibrancy);
  root.classList.toggle('mac', view.platform === 'macos');
  renderSidebar();
  renderBanners();
  renderMain();
  renderToast(view);
}

let sidebarSignature = '';

/** Areas on the left, settings and version below; like KeyZapper. */
function renderSidebar() {
  const attention = view.repositories.filter((r) => r.health.state === 'blocked' || r.health.state === 'needsAttention').length;
  const decisions = view.jobs.filter((j) => j.status.state === 'needsApproval').length;
  const open = openJobs().length;
  const areas = view.repositories.length > 0 ? [
    ['repos', 'Repositories', 'repo', attention ? [attention, 'warning', `${attention} brauchen Aufmerksamkeit`] : [view.repositories.length || null]],
    ['runs', 'Aufträge', 'jobs', decisions ? [decisions, 'decision', `${decisions} warten auf eine Entscheidung`] : [open || null, null, open ? `${open} offen` : null]],
  ] : [];
  const signature = JSON.stringify([area, areas, !!view.running, view.version, view.devBuild, view.checkingUpdate]);
  if (signature === sidebarSignature) return;
  sidebarSignature = signature;
  const check = (beta) => () => call('check_for_updates', { beta });
  document.getElementById('sidebar').replaceChildren(
    h('div', { class: 'brand', 'data-tauri-drag-region': '' }, h('img', { class: 'logo', src: 'icon.svg', alt: '' }), 'OpenReleasePilot'),
    ...areas.map(([id, label, symbol, [count, tone, text]]) => h('button', {
      type: 'button', class: `nav-item${area === id ? ' active' : ''}`, 'aria-current': area === id ? 'page' : null,
      onclick: () => showArea(id),
    }, icon(symbol), h('span', null, label),
    count != null && h('span', { class: `nav-badge ${tone ?? ''}`, title: text, 'aria-label': text }, count))),
    h('div', { class: 'sidebar-footer' },
      h('button', {
        type: 'button', class: 'nav-item', disabled: !!view.running, title: 'Zugänge und Ausführungsregeln', onclick: settingsDialog,
      }, icon('gear'), h('span', null, 'Einstellungen')),
      h('div', { class: 'version' },
        view.devBuild ? 'OpenReleasePilot Entwicklungsversion' : `OpenReleasePilot ${view.version}`,
        view.checkingUpdate ? h('span', null, 'Suche nach Updates …') : button('Nach Updates suchen', check(false), { cls: 'link' }))),
  );
}

/** Title, one line of state and the view's actions above each area: one primary action at most,
 *  frequent ones as icons, rare and destructive ones in "⋯". The open details fill it themselves. */
const pageHead = h('header', { class: 'page-header', 'data-tauri-drag-region': '' });
// Set by the area renderers: { back: [label, action, id], title, chip, subtitle, actions }.
const headerParts = { repos: null, runs: null };

function renderPageHeader() {
  const count = (n, one, many) => `${n} ${n === 1 ? one : many}`;
  let title;
  let subtitle;
  if (area === 'runs') {
    const open = openJobs();
    const decisions = open.filter((j) => j.status.state === 'needsApproval').length;
    title = 'Aufträge';
    subtitle = open.length
      ? [count(open.length, 'offener Auftrag', 'offene Aufträge'), decisions && count(decisions, 'wartet auf eine Entscheidung', 'warten auf eine Entscheidung')].filter(Boolean).join(' · ')
      : 'Keine offenen Aufträge';
  } else {
    const n = (state) => view.repositories.filter((r) => r.health.state === state).length;
    title = 'Repositories';
    subtitle = [count(view.repositories.length, 'Repository', 'Repositories'),
      n('blocked') && `${n('blocked')} blockiert`,
      n('needsAttention') && count(n('needsAttention'), 'braucht Aufmerksamkeit', 'brauchen Aufmerksamkeit'),
      n('healthy') && `${n('healthy')} in Ordnung`].filter(Boolean).join(' · ');
  }
  const parts = headerParts[area] ?? {};
  const drag = { 'data-tauri-drag-region': '' };
  const actions = (parts.actions ?? []).filter(Boolean);
  pageHead.classList.toggle('with-back', !!parts.back);
  pageHead.replaceChildren(...[
    h('div', { class: 'page-title', ...drag },
      parts.back && button(parts.back[0], parts.back[1], { id: parts.back[2], iconName: 'back', cls: 'link page-back', title: 'Zurück zur Übersicht' }),
      h('div', { class: 'page-title-line', ...drag }, h('h1', { class: 'ellipsis', title: parts.title, ...drag }, parts.title ?? title), parts.chip),
      (parts.title ? parts.subtitle : subtitle) && h('p', drag, parts.title ? parts.subtitle : subtitle)),
    actions.length > 0 && h('div', { class: 'page-actions' }, actions),
  ].filter(Boolean));
}

function showArea(id) {
  const main = document.getElementById('main');
  if (area === id) {
    if (id === 'repos' && repoKey) closeRepo();
    if (id === 'runs' && jobId) closeJob();
    return;
  }
  if (area === 'repos' && !repoKey) repoListScroll = main.scrollTop;
  if (area === 'runs' && !jobId) jobListScroll = main.scrollTop;
  area = id;
  renderSidebar();
  renderBanners();
  renderMain();
  if (id === 'repos') main.scrollTop = repoKey ? 0 : repoListScroll;
  if (id === 'runs') main.scrollTop = jobId ? 0 : jobListScroll;
}


function renderBanners() {
  const items = [];
  if (view.availableUpdate) items.push(updateBanner(view, 'OpenReleasePilot'));
  if (view.error) items.push(banner('error', view.error, [['Schließen', () => call('dismiss')]]));
  // A website set up before OpenSitePilot existed: say once where it went.
  if (view.config.projectDir && localStorage.getItem('sitepilot-hint') !== 'seen') {
    items.push(banner('info', 'Die Website aktualisiert und veröffentlicht jetzt die eigene App OpenSitePilot; sie übernimmt Projektordner, SFTP-Zugang und Upload-Stand beim ersten Start.', [
      ['OpenSitePilot laden', () => call('open_url', { url: 'https://github.com/bl0rb/OpenReleasePilot/releases/latest' })],
      ['Ausblenden', () => { localStorage.setItem('sitepilot-hint', 'seen'); renderBanners(); }],
    ]));
  }
  if (view.jobsUnreadable.length) items.push(banner('warning', `Gespeicherte Aufträge nicht lesbar: ${view.jobsUnreadable.join('; ')}`));
  if (view.signedIn === false && view.repositories.some((r) => forgeOf(r) === 'github')) {
    items.push(banner('warning', 'Die GitHub-CLI ist nicht angemeldet: Status nur für öffentliche Repositories, Pflege nicht möglich. Im Terminal: gh auth login, oder ein eigenes Token in den Einstellungen.'));
  }
  for (const host of (view.gitlabHosts ?? []).filter((g) => g.signedIn === false && view.repositories.some((r) => forgeOf(r) === `gitlab:${g.host}`))) {
    items.push(banner('warning', `Die GitLab-CLI ist für ${host.host} nicht angemeldet: Status nur für öffentliche Projekte, Pflege nicht möglich. Im Terminal: glab auth login --hostname ${host.host}, oder ein eigenes Token in den Einstellungen.`));
  }
  document.getElementById('banners').replaceChildren(...items);
}

let reposUi = null;
let runsUi = null;

function renderMain() {
  const main = document.getElementById('main');
  if (!view.repositories.length) {
    reposUi = null;
    runsUi = null;
    area = 'repos';
    main.replaceChildren(h('div', { class: 'empty' },
      h('img', { src: 'icon.svg', alt: '' }),
      h('h2', null, 'Willkommen'),
      h('p', { class: 'muted' }, 'OpenReleasePilot pflegt die Repositories, die du auswählst, auf GitHub und GitLab: Zustand, Pull und Merge Requests, Releases, README und Wiki. Du triffst die Entscheidungen, die App erledigt die Arbeit. Websites aktualisiert und veröffentlicht OpenSitePilot.'),
      h('div', { class: 'line' },
        button('Repositories auswählen …', addReposDialog, { cls: 'primary', iconName: 'plus' }))));
    return;
  }
  if (!reposUi) {
    buildRepos();
    buildRuns();
  }
  renderPageHeader();
  const roots = [pageHead, reposUi.root, runsUi.root];
  if (main.children.length !== roots.length || roots.some((r, i) => main.children[i] !== r)) main.replaceChildren(...roots);
  reposUi.root.hidden = area !== 'repos';
  runsUi.root.hidden = area !== 'runs';
  if (area === 'repos') {
    renderRepos();
    readReposOnce();
  } else {
    renderRuns();
  }
}


// MARK: Repositories

const HEALTH = {
  blocked: ['error', 'Blockiert'],
  needsAttention: ['warning', 'Aufmerksamkeit nötig'],
  unknown: ['idle', 'Unbekannt'],
  healthy: ['ok', 'In Ordnung'],
};
const CI_TEXT = { passed: 'erfolgreich', failed: 'fehlgeschlagen', pending: 'läuft noch', noChecks: 'keine Checks gefunden', unknown: 'unklar' };
const LOCAL_TEXT = {
  notConfigured: 'keine lokale Kopie eingetragen', found: 'lokale Kopie vorhanden', missing: 'lokale Kopie fehlt', foreign: 'lokale Kopie ungültig',
};
const RISKS = { green: ['green', 'Risiko grün'], yellow: ['orange', 'Risiko gelb'], red: ['red', 'Risiko rot'] };
const JOB_STATE = {
  planned: ['idle', 'Geplant'], preparing: ['running', 'In Vorbereitung'], needsApproval: ['warning', 'Freigabe nötig'], running: ['running', 'Läuft'],
  waiting: ['idle', 'Wartet'], succeeded: ['ok', 'Erfolgreich'], partial: ['warning', 'Teilweise erfolgreich'], blocked: ['error', 'Blockiert'],
  failed: ['error', 'Fehlgeschlagen'], cancelled: ['idle', 'Abgebrochen'],
};
const STEP_STATE = {
  pending: ['idle', 'Geplant'], running: ['running', 'Läuft'], needsApproval: ['warning', 'Freigabe nötig'], approved: ['idle', 'Freigegeben'],
  waiting: ['idle', 'Wartet'], done: ['ok', 'Erledigt'], nothingToDo: ['ok', 'Nichts zu tun'], partial: ['warning', 'Teilweise'],
  skipped: ['warning', 'Übersprungen'], failed: ['error', 'Fehlgeschlagen'], rejected: ['idle', 'Verworfen'], blocked: ['error', 'Blockiert'],
  cancelled: ['idle', 'Abgebrochen'],
};
const badge = (cls, text, title) => h('span', { class: `badge ${cls}`, title }, text);
/** The value of a read part of the facts (`{ state: 'ok', value }`), else undefined. */
const ok = (fetch) => (fetch?.state === 'ok' ? fetch.value : undefined);
// Pull request details asked for once per reading of a repository.
const detailsAsked = new Set();

function jobChip(status, running) {
  const [cls, text] = JOB_STATE[status.state] ?? ['idle', status.state];
  return h('span', { class: `chip ${cls}`, title: status.reason }, running && h('span', { class: 'spinner' }), text);
}

/** A step's state in colour and words. A care plan with nothing to do but PRs Claude keeps open is not done: they wait for the user. */
function stepLook(job, step) {
  if (step.kind === 'care' && step.state === 'nothingToDo') {
    // The job list carries the count; the full job has the plan.
    const open = step.waiting ?? (job.plan?.plans ?? []).reduce((n, p) => n + p.prs.filter((pr) => pr.action === 'keep' && !pr.optional).length, 0);
    if (open > 0) return ['warning', `${open} ${open === 1 ? 'braucht' : 'brauchen'} dich`];
  }
  return STEP_STATE[step.state] ?? ['idle', step.state];
}

function stepChip(job, step) {
  const [cls, text] = stepLook(job, step);
  return h('span', { class: `chip ${cls}` }, step.state === 'running' && h('span', { class: 'spinner' }), text);
}

// MARK: Repositories: hosting services

// The services the repositories live on, one card each. GitLab has no connection yet: its card says so
// and explains how to connect, it never pretends to have repositories.
const FORGE_MARKS = {
  github: '<svg viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z"/></svg>',
  gitlab: '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="#e24329" d="m12 21.6 4.2-12.9H7.8z"/><path fill="#fc6d26" d="M12 21.6 7.8 8.7H1.9z"/><path fill="#fca326" d="m1.9 8.7-1.3 3.9c-.1.4 0 .8.3 1L12 21.6z"/><path fill="#e24329" d="M1.9 8.7h5.9L5.3 1c-.1-.4-.7-.4-.8 0z"/><path fill="#fc6d26" d="m12 21.6 4.2-12.9h5.9z"/><path fill="#fca326" d="m22.1 8.7 1.3 3.9c.1.4 0 .8-.3 1L12 21.6z"/><path fill="#e24329" d="M22.1 8.7h-5.9L18.7 1c.1-.4.7-.4.8 0z"/></svg>',
};
/** The GitLab host of a repository id (`host/group/name`), or null for GitHub (`owner/name`). */
function gitlabHost(repo) {
  const first = String(repo ?? '').split('/')[0];
  return /[.:]/.test(first) ? first.toLowerCase() : null;
}
/** The card a repository belongs to: `github` or `gitlab:<host>`. */
const forgeOf = (r) => (r?.forge?.kind === 'gitlab' ? `gitlab:${r.forge.host}` : 'github');
const serviceOf = (repo) => (gitlabHost(repo) ? 'GitLab' : 'GitHub');
const requestWord = (repo) => (gitlabHost(repo) ? 'MR' : 'PR');
const requestsTitle = (repo) => (gitlabHost(repo) ? 'Merge Requests' : 'Pull Requests');
/** `#12` on GitHub, `!12` on GitLab. */
const requestRef = (repo, n) => `${gitlabHost(repo) ? '!' : '#'}${n}`;
const botOf = (repo) => (gitlabHost(repo) ? 'Renovate' : 'Dependabot');
/** Owner or group path and name, without the host. */
function repoParts(repo) {
  const host = gitlabHost(repo);
  const path = host ? repo.slice(host.length + 1) : repo;
  const cut = path.lastIndexOf('/');
  return { host, owner: path.slice(0, cut), name: path.slice(cut + 1), path };
}
// The open service, or null: then the cards are large and nothing is open below them.
let forge = (() => { try { return localStorage.getItem('forge') || null; } catch { return null; } })();

/** GitHub, then one card per GitLab instance (of a managed repository, or one glab is signed in to). */
function forges() {
  const repos = view.repositories;
  const github = repos.filter((r) => forgeOf(r) === 'github');
  const owners = [...new Set(github.map((r) => repoParts(r.repo).owner))];
  const cards = [
    { id: 'github', kind: 'github', name: 'GitHub', where: ['github.com', owners.slice(0, 2).join(', ') + (owners.length > 2 ? ` +${owners.length - 2}` : '')].filter(Boolean).join(' · '),
      repos: github, connected: view.signedIn !== false || view.config.githubTokenSaved,
      login: view.config.githubTokenSaved ? 'eigenes Token' : view.signedIn === false ? 'gh nicht angemeldet' : view.signedIn ? 'über gh angemeldet' : 'Anmeldung wird geprüft' },
  ];
  for (const h of view.gitlabHosts ?? []) {
    cards.push({
      id: `gitlab:${h.host}`, kind: 'gitlab', host: h.host, name: 'GitLab', where: h.host,
      repos: repos.filter((r) => forgeOf(r) === `gitlab:${h.host}`), connected: h.ownToken || h.signedIn === true || (h.signedIn == null && h.configured),
      login: h.ownToken ? 'eigenes Token' : h.signedIn === true ? 'über glab angemeldet' : h.signedIn === false ? 'glab nicht angemeldet' : h.configured ? 'glab eingerichtet' : 'Anmeldung wird geprüft',
    });
  }
  // Without any instance: one card that explains how to connect one.
  if (cards.length === 1) cards.push({ id: 'gitlab', kind: 'gitlab', name: 'GitLab', where: 'noch keine Instanz', repos: [], connected: false, login: 'nicht verbunden' });
  return cards;
}

function selectForge(id) {
  forge = forge === id ? null : id;
  try { forge ? localStorage.setItem('forge', forge) : localStorage.removeItem('forge'); } catch { /* a convenience only */ }
  renderRepos();
  // The content below slides in; the card keeps the focus.
  reposUi.below.classList.remove('enter');
  void reposUi.below.offsetWidth;
  if (forge) reposUi.below.classList.add('enter');
}

/** One card per service: mark, account, login, a dot per repository, the counts that need you. */
function renderForges() {
  const list = forges();
  reposUi.forges.classList.toggle('compact', !!forge);
  reposUi.forges.replaceChildren(...list.map((f) => {
    const n = (state) => f.repos.filter((r) => r.health.state === state).length;
    const decisions = view.jobs.filter((j) => j.status.state === 'needsApproval' && j.repos.some((k) => f.repos.some((r) => r.key === k))).length;
    const counts = [
      [f.repos.length, f.repos.length === 1 ? 'Repository' : 'Repositories', ''],
      [n('needsAttention'), 'Aufmerksamkeit', 'orange'], [n('blocked'), 'blockiert', 'red'], [decisions, decisions === 1 ? 'Entscheidung' : 'Entscheidungen', 'accent'],
    ].filter(([count], i) => i === 0 || count > 0);
    const on = forge === f.id;
    return h('button', {
      type: 'button', id: domId('forge', f.id), class: `forge-card ${f.kind} ${on ? 'on' : ''} ${f.connected ? '' : 'offline'}`,
      'aria-pressed': String(on), 'aria-controls': 'repos-below', onclick: () => selectForge(f.id),
      title: on ? 'Wieder alle Dienste zeigen' : `${f.name} öffnen`,
    },
      h('span', { class: 'forge-head' },
        h('span', { class: 'forge-mark' }, (() => { const m = document.createElement('span'); m.innerHTML = FORGE_MARKS[f.kind]; return m.firstChild; })()),
        h('span', { class: 'forge-title' }, h('strong', null, f.name), h('span', { class: 'muted small', title: f.where }, f.where)),
        h('span', { class: 'spacer' }),
        h('span', { class: `chip ${f.connected ? 'ok' : 'idle'}` }, f.login)),
      h('span', { class: 'forge-more' }, h('span', { class: 'forge-more-inner' },
        f.repos.length ? h('span', { class: 'forge-dots', role: 'img', 'aria-label': `Zustand: ${counts.map(([c, t]) => `${c} ${t}`).join(', ')}` },
          f.repos.map((r) => h('span', { class: `forge-dot ${r.loading && !r.facts ? 'idle' : HEALTH[r.health.state][0]}`, title: `${r.repo}: ${HEALTH[r.health.state][1]}` })))
          : h('span', { class: 'muted small' }, f.connected ? 'Noch keine Repositories' : f.host ? `Mit glab anmelden (glab auth login --hostname ${f.host}) oder ein eigenes Token in den Einstellungen` : 'Noch nicht verbunden: Karte öffnen für die Einrichtung'),
        h('span', { class: 'forge-counts' }, counts.map(([c, t, cls]) => h('span', { class: cls || null }, h('strong', null, c), ` ${t}`))))),
      h('span', { class: 'forge-compact-count muted small' }, f.repos.length ? `${f.repos.length}` : ''));
  }));
}

/** How a GitLab instance gets connected: below the card while there is none, or while glab is not signed in. */
function gitlabPanel(host) {
  const target = host ?? '<deine-instanz>';
  return h('section', { class: 'panel section forge-setup' },
    h('h2', null, host ? `GitLab ${host} verbinden` : 'GitLab verbinden'),
    h('p', { class: 'muted', style: 'margin:0' }, 'Die App liest und pflegt GitLab-Projekte wie GitHub-Repositories: Status, Merge Requests mit Renovate, Releases, README und Wiki, Sicherheitsprüfung. Angemeldet wird über die GitLab-CLI (die App nutzt ihren Token) oder mit einem eigenen Token unter Einstellungen → Zugänge → „Instanz mit Token hinzufügen“; Claude bekommt den Token nie.'),
    h('ol', { class: 'plain-list steps-list' },
      h('li', null, 'GitLab-CLI installieren: ', h('code', { class: 'selectable' }, 'brew install glab')),
      h('li', null, 'An der Instanz anmelden: ', h('code', { class: 'selectable' }, `glab auth login --hostname ${target}`)),
      h('li', null, 'Projekte aufnehmen: Lokale Klone im Repo-Ordner erscheinen unter „Repositories auswählen“; ohne Klon dort ', h('code', { class: 'selectable' }, `${target}/gruppe/projekt`), ' eingeben.'),
      h('li', null, '„Status aktualisieren“, damit die App die Anmeldung erneut prüft.')),
    h('div', { class: 'line' }, button('Repositories auswählen …', addReposDialog, { cls: 'primary', iconName: 'plus' })));
}

// Overview filter: a name search and one of the repository states; never changes the selection.
const REPO_FILTERS = [
  ['all', 'Alle', () => true],
  ['attention', 'Aufmerksamkeit', (r) => r.health.state === 'needsAttention'],
  ['blocked', 'Blockiert', (r) => r.health.state === 'blocked'],
  ['unknown', 'Unbekannt', (r) => r.health.state === 'unknown'],
];
let repoQuery = '';
let repoFilter = 'all';

function buildRepos() {
  reposUi = {
    root: h('div', { class: 'area repos-area' }),
    decisions: h('section', { class: 'section', 'aria-label': 'Offene Aufträge' }),
    overview: h('section', { class: 'section' }),
    head: h('div', { class: 'section-head wrap' }),
    note: h('div', { class: 'note' }),
    // Built once, so typing in the search keeps focus and caret.
    search: h('input', {
      type: 'search', id: 'repos-search', placeholder: 'Repositories suchen …', spellcheck: 'false', autocomplete: 'off', 'aria-label': 'Repositories nach Namen suchen',
      oninput: (e) => { repoQuery = e.target.value.trim().toLowerCase(); renderRepos(); },
    }),
    filters: h('div', { class: 'segmented', role: 'group', 'aria-label': 'Nach Zustand filtern' }),
    body: h('div', { class: 'repos-body' }),
  };
  reposUi.filterBar = h('div', { class: 'filter-bar' }, reposUi.search, reposUi.filters);
  reposUi.overview.append(reposUi.head, reposUi.note, reposUi.filterBar);
  // Built once, so selecting a card animates instead of rebuilding.
  reposUi.forges = h('div', { class: 'forges', role: 'group', 'aria-label': 'Dienste' });
  reposUi.below = h('div', { class: 'repos-below', id: 'repos-below' }, reposUi.overview, reposUi.body);
  reposUi.root.append(reposUi.forges, reposUi.decisions, reposUi.below);
}

const repoVisible = (r) => (REPO_FILTERS.find(([id]) => id === repoFilter)?.[2] ?? (() => true))(r)
  && (!repoQuery || r.repo.toLowerCase().includes(repoQuery));

function resetRepoFilter() {
  repoQuery = '';
  repoFilter = 'all';
  reposUi.search.value = '';
  renderRepos();
  reposUi.search.focus();
}

/** Reads every repository once when the area first opens; later only on request. */
function readReposOnce() {
  if (reposRead || !view.repositories.length) return;
  reposRead = true;
  const unread = view.repositories.filter((r) => !r.facts && !r.loading).map((r) => r.key);
  if (unread.length) call('refresh_repos', { keys: unread });
}

/** Re-renders, then gives the focus back to the element with the same id. */
function keepFocus(render) {
  const id = document.activeElement?.id;
  render();
  if (id && document.activeElement?.id !== id) document.getElementById(id)?.focus({ preventScroll: true });
}

function renderRepos() {
  keepFocus(() => {
    const repos = view.repositories.filter((r) => !forge || forgeOf(r) === forge);
    // A repository opened from elsewhere (a job) shows whichever service is open.
    const repo = repoKey && view.repositories.find((r) => r.key === repoKey);
    if (!repo) repoKey = null;
    // The details concern one repository only: no service cards, no fleet actions, no other repository's jobs.
    reposUi.overview.hidden = !!repo;
    reposUi.forges.hidden = !!repo;
    if (repo) {
      reposUi.decisions.hidden = true;
      reposUi.body.replaceChildren(repoDetail(repo));
      headerParts.repos = repoDetailHeader(repo);
      renderPageHeader();
      return;
    }
    const card = forges().find((f) => f.id === forge);
    if (forge && !card) forge = null;
    renderForges();
    if (!card || (card.kind === 'gitlab' && !card.host)) {
      // No service open, or no GitLab instance yet: the open decisions stay, the setup below.
      reposUi.overview.hidden = true;
      renderDecisions();
      reposUi.body.replaceChildren(...(card ? [gitlabPanel()] : []));
      headerParts.repos = null;
      renderPageHeader();
      return;
    }
    // An instance glab is not signed in to: how to sign in, then its repositories as far as readable.
    const signIn = card.kind === 'gitlab' && !card.connected && gitlabPanel(card.host);
    const reading = repos.some((r) => r.loading || r.detailsLoading);
    const securing = repos.some((r) => r.securityLoading);
    const chosen = repos.filter((r) => selection.has(r.key));
    const visible = repos.filter(repoVisible);
    const careable = visible.filter((r) => !r.capabilities.care && !r.job);
    const hiddenChosen = chosen.filter((r) => !repoVisible(r)).length;
    const allVisible = careable.length > 0 && careable.every((r) => selection.has(r.key));
    const filtered = repoQuery || repoFilter !== 'all';
    reposUi.head.replaceChildren(...[
      repos.length > 0 && h('label', { class: 'check-all', title: filtered ? 'Alle sichtbaren pflegbaren Repositories an- oder abwählen' : 'Alle pflegbaren Repositories an- oder abwählen' },
        h('input', {
          type: 'checkbox', id: 'repos-select-all', checked: allVisible,
          indeterminate: !allVisible && careable.some((r) => selection.has(r.key)), disabled: !careable.length,
          onchange: (e) => { for (const r of careable) e.target.checked ? selection.add(r.key) : selection.delete(r.key); renderRepos(); },
        }), filtered ? 'Alle Treffer' : 'Alle'),
      h('span', { class: 'muted small', role: 'status' }, repos.length
        ? [`${repos.length} verwaltet`, filtered && `${visible.length} angezeigt`,
          chosen.length && `${chosen.length} ausgewählt${hiddenChosen ? `, davon ${hiddenChosen} ausgeblendet` : ''}`,
          reading && 'Status wird gelesen …', securing && 'Sicherheit wird geprüft …'].filter(Boolean).join(' · ')
        : ''),
      (reading || securing) && button('Abbrechen', () => call('cancel_repo_refresh'), { id: 'repos-cancel-read', cls: 'link', title: 'Laufende Lesevorgänge abbrechen; was schon gelesen ist, bleibt' }),
    ].filter(Boolean));
    reposUi.note.hidden = true;
    headerParts.repos = {
      actions: [
        repos.length > 0 && iconButton('Status aktualisieren', () => call('refresh_repos', { keys: [] }), 'refresh', {
          id: 'repos-refresh', busy: reading,
          title: `Status aktualisieren: liest den Stand aller Repositories auf ${card.name}, nur lesend und ohne Claude. Ändern tut erst ein Pflegeauftrag, und der legt dir vorher jeden Vorschlag zur Freigabe vor.`,
        }),
        menuButton('Weitere Aktionen', () => [
          ['Repositories hinzufügen …', addReposDialog, { iconName: 'plus' }],
          repos.length > 0 && [chosen.length ? `Sicherheit prüfen (${chosen.length} ausgewählt)` : 'Sicherheit aller prüfen', () => call('check_security', { keys: chosen.map((r) => r.key) }), {
            iconName: 'shield', disabled: securing,
            title: 'Schutz des Default-Branches (direkte Pushes, Force-Push), Secret- und Abhängigkeits-Scanning, offene Alerts, Workflow-Rechte. Nur lesend, ohne Claude.',
          }],
        ], { id: 'repos-more' }),
        repos.length > 0 && button(chosen.length ? `Pflegen (${chosen.length}) …` : 'Pflegen …', () => jobDialog(chosen.map((r) => r.key)),
          { id: 'repos-care', iconName: 'branch', cls: 'primary', disabled: !chosen.length || !!view.running,
            title: chosen.length ? `Pflegeauftrag für die ${chosen.length} ausgewählten Repositories${hiddenChosen ? `, auch die ${hiddenChosen} ausgeblendeten` : ''}` : 'Erst Repositories auswählen (Häkchen auf den Karten)' }),
      ],
    };
    renderPageHeader();
    reposUi.filterBar.hidden = repos.length < 2;
    reposUi.filters.replaceChildren(...REPO_FILTERS.map(([id, label, test]) => {
      const n = id === 'all' ? repos.length : repos.filter(test).length;
      return h('button', {
        type: 'button', id: `repos-filter-${id}`, class: `segment ${repoFilter === id ? 'on' : ''}`, 'aria-pressed': String(repoFilter === id),
        onclick: () => { repoFilter = id; renderRepos(); },
      }, label, h('span', { class: 'segment-count' }, n));
    }));
    renderDecisions();
    let body;
    if (!repos.length) {
      body = h('div', { class: 'empty' },
        h('h2', null, 'Repositories auswählen'),
        h('p', { class: 'muted' }, card.kind === 'gitlab'
          ? `Noch kein Projekt von ${card.host}. Lokale Klone erscheinen unter „Repositories auswählen“; ohne Klon dort ${card.host}/gruppe/projekt eingeben.`
          : 'Wähle die Repositories, um die sich die App kümmern soll. Eine Website, ein SFTP-Zugang oder ein Release sind dafür nicht nötig.'),
        h('div', { class: 'line' }, button('Repositories auswählen …', addReposDialog, { cls: 'primary', iconName: 'plus' })));
    } else if (!visible.length) {
      body = h('div', { class: 'note-box' },
        h('span', null, 'Kein Repository passt zu Suche und Filter.', hiddenChosen ? ` Die Auswahl (${hiddenChosen}) bleibt erhalten.` : ''),
        h('div', null, button('Filter zurücksetzen', resetRepoFilter, { id: 'repos-filter-reset', cls: 'link' })));
    } else {
      body = h('div', { class: 'repo-cards' }, visible.map(repoCard));
    }
    // Not signed in and nothing added yet: the guide says it all.
    reposUi.body.replaceChildren(...[signIn, signIn && !repos.length ? null : body].filter(Boolean));
  });
}

/** Open jobs at the top: what needs a decision first, then what runs or waits. */
function renderDecisions() {
  const order = { needsApproval: 0, running: 1, preparing: 1, blocked: 2, waiting: 3, planned: 4 };
  // With a service open, only the jobs of its repositories; without one, all of them.
  const ofForge = (j) => !forge || j.repos.some((k) => view.repositories.some((r) => r.key === k && forgeOf(r) === forge));
  const jobs = openJobs().filter(ofForge).sort((a, b) => (order[a.status.state] ?? 9) - (order[b.status.state] ?? 9));
  reposUi.decisions.hidden = !jobs.length;
  if (!jobs.length) return;
  reposUi.decisions.replaceChildren(
    h('div', { class: 'section-head' }, h('h2', null, jobs.some((j) => j.status.state === 'needsApproval') ? 'Entscheidungen und laufende Aufträge' : 'Laufende Aufträge')),
    h('div', { class: 'job-cards' }, jobs.map(jobCard)));
}

function jobTitle(j) {
  const names = j.repos.map((key) => repoParts(repoName(key)).name);
  return names.length > 3 ? `${names.slice(0, 3).join(', ')} +${names.length - 3}` : names.join(', ');
}

/** The job's steps with their state in words, never by colour alone. */
function stepPills(j, details) {
  return h('ul', { class: 'job-card-steps', 'aria-label': 'Schritte' }, j.steps.map((s) => {
    const [cls, state] = stepLook(j, s);
    return h('li', { class: `step-pill ${cls}`, title: details && s.detail ? `${s.label}: ${state} – ${s.detail}` : null },
      h('span', null, s.label), h('span', { class: 'step-pill-state' }, ` · ${state}`));
  }));
}

function jobCard(j) {
  const decide = j.status.state === 'needsApproval';
  // The whole card opens the job; a button only where a decision waits.
  return h('div', { class: `job-card ${JOB_STATE[j.status.state]?.[0] ?? ''}`, onclick: () => openJob(j.id) },
    h('div', { class: 'job-card-head' },
      h('button', { type: 'button', class: 'repo-name', id: domId('job-open', j.id), onclick: (e) => { e.stopPropagation(); openJob(j.id); } }, h('strong', { class: 'ellipsis' }, jobTitle(j))),
      h('span', { class: 'spacer' }), jobChip(j.status, j.running)),
    h('div', { class: 'job-card-reason selectable' }, j.status.reason),
    stepPills(j, true),
    h('div', { class: 'line' },
      h('span', { class: 'muted small' }, `Auftrag vom ${formatTime(j.createdAt)}`),
      h('span', { class: 'spacer' }),
      decide && button('Entscheiden …', () => openJob(j.id), { cls: 'primary', id: domId('job-decide', j.id) })));
}

function openRepo(key) {
  const main = document.getElementById('main');
  // Back from the details leads to the service the repository lives on.
  const r = view.repositories.find((x) => x.key === key);
  if (r) forge = forgeOf(r);
  repoListScroll = main.scrollTop;
  repoKey = key;
  renderRepos();
  main.scrollTop = 0;
  document.getElementById('repo-back')?.focus({ preventScroll: true });
}

function closeRepo() {
  const key = repoKey;
  repoKey = null;
  manageDrafts.delete(key);
  renderRepos();
  document.getElementById('main').scrollTop = repoListScroll;
  if (key) document.getElementById(domId('repo-row', key))?.focus({ preventScroll: true });
}

// MARK: Repositories: overview

function healthChip(r) {
  if (r.loading && !r.facts) return h('span', { class: 'chip running' }, h('span', { class: 'spinner' }), 'Wird gelesen');
  const [cls, text] = HEALTH[r.health.state];
  return h('span', { class: `chip ${cls}`, title: 'Zustand des Repositorys (unabhängig von Aufträgen)' }, text);
}

function mainReason(r) {
  if (r.loading && !r.facts) return `${serviceOf(r.repo)} wird abgefragt …`;
  return r.health.reasons[0] ?? r.health.gaps[0] ?? 'Alles gelesen, kein Befund';
}

function prSummary(r) {
  const prs = r.facts?.prs;
  if (!prs || prs.state === 'notQueried') return `${requestWord(r.repo)}s nicht abgefragt`;
  if (prs.state === 'failed') return `${requestWord(r.repo)}-Abfrage fehlgeschlagen`;
  const { items, complete } = prs.value;
  const dependabot = items.filter((p) => p.dependabot).length;
  if (!items.length) return `keine offenen ${requestWord(r.repo)}s`;
  const word = requestWord(r.repo);
  return `${complete ? '' : 'mind. '}${items.length} offene${items.length === 1 ? `r ${word}` : ` ${word}s`}${dependabot ? `, ${dependabot} von ${botOf(r.repo)}` : ''}`;
}

function ciSummary(r) {
  const ci = r.facts?.ci;
  const branch = ok(r.facts?.access)?.defaultBranch ?? 'Default-Branch';
  if (!ci || ci.state === 'notQueried') return `CI ${branch} nicht abgefragt`;
  if (ci.state === 'failed') return `CI ${branch}: Abfrage fehlgeschlagen`;
  return `CI ${branch}: ${CI_TEXT[ci.value.state]}`;
}

function releaseSummary(r) {
  const f = r.facts;
  if (!f) return null;
  if (ok(f.release)) return `Release ${f.release.value}`;
  if (f.release.state === 'failed') return 'Release-Abfrage fehlgeschlagen';
  if (ok(f.tag)) return `kein Release, Tag ${f.tag.value}`;
  return f.release.state === 'ok' ? 'kein veröffentlichtes Release' : null;
}

function checkedSummary(r) {
  const f = r.facts;
  if (!f) return r.error ? `nicht gelesen: ${r.error}` : 'noch nicht gelesen';
  return `gelesen ${ago(f.checkedAt)}${r.health.stale ? ', veraltet' : ''}${r.error ? ', letzte Abfrage fehlgeschlagen' : ''}`;
}

/** What the repository feeds: content targets. */
function relations(r) {
  const targets = r.targets.map((id) => view.targets.find((t) => t.id === id)?.name).filter(Boolean);
  return [
    targets.length && `Inhalte: ${targets.join(', ')}`,
    r.website && 'Website-Projekt',
    r.releasedBy ? `Release kommt aus ${repoName(r.releasedBy)}`
      : r.release ? `Release-Regel: Tags ${r.release.tagPrefix}x.y.z${r.release.publishesRelease ? `, mit ${serviceOf(r.repo)}-Release` : ''}` : 'keine Release-Regel',
    releasedThrough(r.key).length > 0 && `veröffentlicht ${releasedThrough(r.key).map((f) => f.repo).join(', ')}`,
  ].filter(Boolean).join(' · ');
}

function repoCard(r) {
  const { owner, name, host } = repoParts(r.repo);
  const locked = r.capabilities.care ?? (r.job ? 'gehört zu einem offenen Auftrag' : null);
  return h('div', { class: `repo-card ${r.loading && !r.facts ? '' : HEALTH[r.health.state][0]} ${r.enabled ? '' : 'off'}`, onclick: () => openRepo(r.key) },
    h('div', { class: 'repo-card-head' },
      h('input', {
        type: 'checkbox', id: domId('repo-pick', r.key), checked: selection.has(r.key) && !locked, disabled: !!locked,
        title: locked ? `Nicht wählbar: ${locked}` : 'Für den nächsten Pflegeauftrag auswählen', 'aria-label': `${r.repo} auswählen`,
        onclick: (e) => e.stopPropagation(),
        onchange: (e) => { e.target.checked ? selection.add(r.key) : selection.delete(r.key); renderRepos(); },
      }),
      h('button', { type: 'button', class: 'repo-name', id: domId('repo-row', r.key), onclick: (e) => { e.stopPropagation(); openRepo(r.key); } },
        h('strong', { class: 'ellipsis' }, name), h('span', { class: 'muted small ellipsis', title: host ? `${host}/${owner}` : owner }, owner)),
      h('span', { class: 'spacer' }),
      r.loading && r.facts && h('span', { class: 'spinner muted', title: 'wird neu gelesen' }),
      r.job && jobChip(r.job.status, r.job.status.state === 'running' || r.job.status.state === 'preparing'),
      healthChip(r)),
    h('span', { class: 'repo-card-reason' }, r.enabled ? mainReason(r) : 'deaktiviert: wird gelesen, aber nicht gepflegt'),
    h('span', { class: 'repo-card-meta' }, (r.facts ? [prSummary(r), ciSummary(r), releaseSummary(r)] : []).concat(LOCAL_TEXT[r.local.state], checkedSummary(r))
      .filter(Boolean).map((text) => h('span', null, text)), securitySummary(r)),
    h('span', { class: 'repo-card-rel' }, relations(r)));
}

// MARK: Repositories: details

// Which folded parts of a repository's details the user opened, `key:part` → open; survives re-rendering.
const repoFold = new Map();

/** A collapsible part of the details that keeps its open state across re-renders. */
function fold(key, part, summary, body, open = false) {
  const id = `${key}:${part}`;
  return h('details', { class: 'panel fold-panel', id: domId('fold', id), open: repoFold.get(id) ?? open, ontoggle: (e) => repoFold.set(id, e.target.open) },
    h('summary', null, summary), h('div', { class: 'fold-body' }, body));
}

/** CI, release and data state in one line, for the top of the details. */
function statusLine(r) {
  if (!r.facts) return h('div', { class: 'facts' }, h('span', null, checkedSummary(r)));
  return h('div', { class: 'facts' }, [ciSummary(r), releaseSummary(r), checkedSummary(r)].filter(Boolean).map((text, i) =>
    h('span', { class: i === 2 && r.health.stale ? 'orange' : null }, text)));
}

function repoDetail(r) {
  const f = r.facts;
  const access = ok(f?.access);
  const branch = access?.defaultBranch ?? 'Default-Branch';
  if (f && !r.details && !r.detailsLoading && ok(f.prs)?.items.length) {
    const mark = `${r.key}:${f.checkedAt}`;
    if (!detailsAsked.has(mark)) {
      detailsAsked.add(mark);
      queueMicrotask(() => call('load_repo_details', { key: r.key }));
    }
  }
  // Security findings that matter go up front; otherwise the check follows the content targets.
  const urgent = r.security?.checks.some((c) => c.level === 'high' || c.level === 'medium');
  const security = h('section', { class: `panel section ${urgent ? 'urgent' : ''}`, id: 'repo-security-section' }, securitySection(r));
  return h('div', { class: 'repo-detail' },
    statusLine(r),
    r.job && h('div', { class: `note-box ${JOB_STATE[r.job.status.state]?.[0] ?? ''}` },
      h('span', null, h('strong', null, 'Offener Auftrag: '), jobChip(r.job.status, false), h('span', { class: 'selectable' }, ` ${r.job.status.reason}`)),
      h('span', { class: 'muted small' }, 'Pflege, Sicherheitskorrekturen und Verwaltung dieses Repositorys sind gesperrt, bis der Auftrag abgeschlossen oder beendet ist.'),
      h('div', null, button('Auftrag ansehen', () => openJob(r.job.id), { id: 'repo-job', cls: 'link' }))),
    findings(r),
    urgent && security,
    h('section', { class: 'panel section' }, prSection(r, branch)),
    h('section', { class: 'panel section' }, contentSection(r)),
    !urgent && security,
    fold(r.key, 'analysis', analysisSummary(r), analysisSection(r)),
    fold(r.key, 'tech', 'Technische Details', statusFacts(r, branch)),
    fold(r.key, 'manage', 'Verwaltung', manageSection(r), manageDrafts.get(r.key)?.dirty));
}

/** The details' page header: back to the service, name and state, refresh, open, care. */
function repoDetailHeader(r) {
  const access = ok(r.facts?.access);
  const locked = r.capabilities.care ?? (r.job ? 'gehört zu einem offenen Auftrag' : null);
  return {
    back: [gitlabHost(r.repo) ? `GitLab ${gitlabHost(r.repo)}` : 'GitHub', closeRepo, 'repo-back'],
    title: repoParts(r.repo).path,
    chip: healthChip(r),
    actions: [
      iconButton('Status aktualisieren', () => call('refresh_repos', { keys: [r.key] }), 'refresh',
        { id: 'repo-refresh', busy: r.loading, title: `Status aktualisieren: liest den Stand von ${r.repo} auf ${serviceOf(r.repo)}, nur lesend und ohne Claude` }),
      access?.url && iconButton(`Auf ${serviceOf(r.repo)} öffnen`, () => call('open_url', { url: access.url }), 'external', { id: 'repo-github' }),
      button('Pflegen …', () => jobDialog([r.key]), { id: 'repo-care', cls: 'primary', iconName: 'branch', disabled: !!locked || !!view.running, title: locked ? `Nicht möglich: ${locked}` : `Pflegeauftrag für ${r.repo}` }),
    ],
  };
}

function findings(r) {
  const { state, reasons, gaps, notes } = r.health;
  const list = (title, items, cls) => items.length > 0 && h('div', { class: 'finding' },
    h('strong', { class: `small ${cls}` }, title), h('ul', null, items.map((text) => h('li', { class: 'selectable' }, text))));
  const blocks = [
    list('Befunde', reasons, state === 'blocked' ? 'red' : 'orange'),
    list('Datenlücken', gaps, ''),
    list('Hinweise', notes, 'muted'),
    r.error && h('div', { class: 'note orange' }, `Letzte Abfrage fehlgeschlagen, gezeigt wird der ältere Stand: ${r.error}`),
  ].filter(Boolean);
  if (!reasons.length && !gaps.length && r.facts) blocks.unshift(h('div', { class: 'note green' }, 'Alles gelesen, kein Befund.'));
  return h('div', { class: 'findings' }, blocks);
}

function fetchText(fetch, render) {
  if (!fetch) return 'noch nicht gelesen';
  if (fetch.state === 'failed') return `Abfrage fehlgeschlagen: ${fetch.value}`;
  if (fetch.state === 'notQueried') return `nicht abgefragt${fetch.value ? `: ${fetch.value}` : ''}`;
  return render(fetch.value);
}

function releaseText(f) {
  if (f.release.state === 'failed') return `Abfrage fehlgeschlagen: ${f.release.value}`;
  if (ok(f.release)) return `${f.release.value} (veröffentlichtes Release)`;
  if (f.release.state !== 'ok') return 'nicht abgefragt';
  if (ok(f.tag)) return `kein veröffentlichtes Release · höchster passender Tag ${f.tag.value}`;
  if (f.tag.state === 'failed') return `kein veröffentlichtes Release · Tags: ${f.tag.value}`;
  return 'kein veröffentlichtes Release, kein passender Tag';
}

function localText(local) {
  switch (local.state) {
    case 'found': return local.path;
    case 'missing': return `fehlt: ${local.path}`;
    case 'foreign': return `${local.path} ist keine Kopie dieses Repositorys (${local.origin ?? 'ohne erkannten origin'})`;
    default: return 'nicht eingetragen; Status und PR-Pflege gehen auch ohne';
  }
}

function statusFacts(r, branch) {
  const f = r.facts;
  const access = ok(f?.access);
  const row = (label, value, cls = '') => [h('span', { class: 'label' }, label), h('span', { class: `detail-text selectable ${cls}` }, value)];
  const since = f?.since;
  const sinceLabel = ok(since) ? `Commits seit ${since.value.baseKind === 'release' ? 'Release' : 'Tag'} ${since.value.base}` : 'Commits seit Release';
  return h('div', { class: 'fact-grid' },
    row('Repository', access ? [access.private ? 'privat' : 'öffentlich', access.archived && 'archiviert'].filter(Boolean).join(' · ') : fetchText(f?.access, () => '')),
    access && row('Schreibzugriff', access.canPush == null ? `unbekannt (ohne ${gitlabHost(r.repo) ? 'glab' : 'gh'}-Anmeldung meldet ${serviceOf(r.repo)} keine Rechte)` : access.canPush ? 'ja' : 'nein, nur lesbar', access.canPush === false ? 'orange' : ''),
    row('Default-Branch', access ? `${branch}${ok(f.head) ? ` · HEAD ${short(f.head.value)}` : ''}` : '–'),
    row('CI', fetchText(f?.ci, (ci) => `${CI_TEXT[ci.state]}: ${ci.detail} (Commit ${short(ci.sha)})`),
      ok(f?.ci)?.state === 'failed' ? 'red' : ok(f?.ci)?.state === 'passed' ? 'green' : ''),
    row('Release', f ? releaseText(f) : '–'),
    row(sinceLabel, !since ? '–' : ok(since) ? String(since.value.commits) : since.state === 'failed' ? since.value : `nicht bestimmbar${since.value ? `: ${since.value}` : ''}`),
    r.releasedBy
      ? row('Release', `kommt aus ${repoName(r.releasedBy)}: die App taggt hier nie, sie wartet auf das Release, das ein Tag dort hier veröffentlicht`)
      : row('Release-Regel', r.release ? `Tags ${r.release.tagPrefix}x.y.z${r.release.publishesRelease ? `, danach wartet die App auf das ${serviceOf(r.repo)}-Release` : ''}` : 'keine: die App setzt keine Tags', r.release ? '' : 'muted'),
    releasedThrough(r.key).length > 0 && row('Veröffentlicht', `${releasedThrough(r.key).map((f) => f.repo).join(', ')}: ein Tag hier bringt dort das Release mit`),
    row('Lokale Kopie', localText(r.local), ['found', 'notConfigured'].includes(r.local.state) ? '' : 'orange'),
    r.website && row('Rolle', 'Website-Projekt (site.json); aktualisiert und veröffentlicht wird es mit OpenSitePilot'),
    row('Gelesen', f ? `${formatTime(f.checkedAt)} (${ago(f.checkedAt)})${r.health.stale ? ', veraltet' : ''}` : 'noch nicht', r.health.stale ? 'orange' : ''));
}

function prSection(r, branch) {
  const prs = r.facts?.prs;
  const list = ok(prs);
  const head = (info) => h('div', { class: 'section-head wrap' }, h('h3', null, requestsTitle(r.repo)), info, h('span', { class: 'spacer' }),
    list?.items.length > 0 && button(r.detailsLoading ? 'Wird gelesen …' : 'CI und Mergebarkeit neu lesen', () => call('load_repo_details', { key: r.key }),
      { id: 'repo-details', iconName: 'refresh', disabled: r.detailsLoading }));
  if (!list) return [head(), h('div', { class: 'note' }, fetchText(prs, () => ''))];
  if (!list.items.length) return [head(), h('div', { class: 'note' }, `Keine offenen ${requestsTitle(r.repo)}.`)];
  const dependabot = list.items.filter((p) => p.dependabot).length;
  const intoDefault = list.items.filter((p) => p.intoDefault).length;
  const details = new Map((r.details?.prs ?? []).map((d) => [d.number, d]));
  return [
    head(h('span', { class: 'muted small' },
      `${list.complete ? '' : 'mindestens '}${list.items.length} offen, davon ${dependabot} von ${botOf(r.repo)}; ${intoDefault} auf ${branch} kommen für die Pflege in Frage`)),
    r.detailsLoading && !r.details && h('div', { class: 'note' }, 'CI und Mergebarkeit werden gelesen …'),
    r.details && !r.details.complete && h('div', { class: 'note' }, `CI und Mergebarkeit gelesen für die ersten ${r.details.prs.length}.`),
    h('div', { class: 'items' }, list.items.map((pr) => prItem(r.repo, pr, details.get(pr.number), branch))),
  ].filter(Boolean);
}

function prItem(repo, pr, detail, branch) {
  const ci = detail?.ci;
  const ciBadge = !detail ? null : ok(ci)
    ? badge({ passed: 'green', failed: 'red', pending: '', noChecks: 'orange', unknown: 'orange' }[ci.value.state], `CI ${CI_TEXT[ci.value.state]}`, ci.value.detail)
    : badge('orange', 'CI nicht abfragbar', ci?.value);
  const merge = detail && badge(['clean', 'has_hooks'].includes(detail.mergeableState) ? 'green' : detail.mergeableState === 'dirty' ? 'red' : '', detail.mergeable);
  return h('div', { class: 'item' },
    h('div', { class: 'item-line' },
      button(requestRef(repo, pr.number), () => call('open_url', { url: pr.url }), { cls: 'link mono', title: `Auf ${serviceOf(repo)} öffnen` }),
      h('span', { class: 'item-title', title: pr.title }, pr.title),
      pr.dependabot ? badge('accent', botOf(repo)) : h('span', { class: 'muted small' }, pr.author),
      pr.draft && badge('', 'Entwurf'),
      h('span', { class: 'spacer' }),
      ciBadge, merge),
    h('div', { class: 'item-reason' }, [
      pr.intoDefault ? `Ziel ${pr.base}` : `Ziel ${pr.base}, nicht ${branch}: kommt nicht in den Pflegeplan`,
      !detail && 'CI und Mergebarkeit noch nicht gelesen',
    ].filter(Boolean).join(' · ')));
}

// MARK: Repositories: security

const SEC_LEVEL = {
  high: ['red', 'Kritisch'], medium: ['orange', 'Wichtig'], low: ['', 'Hinweis'], unknown: ['', 'Nicht prüfbar'], ok: ['green', 'In Ordnung'],
};
const FINDING_LEVELS = ['high', 'medium', 'low'];

/** One line on the card: what the last security check found. */
function securitySummary(r) {
  if (r.securityLoading && !r.security) return h('span', null, 'Sicherheit wird geprüft …');
  if (!r.security) return null;
  const [high, medium, low] = FINDING_LEVELS.map((level) => r.security.checks.filter((c) => c.level === level).length);
  const parts = [high && `${high} kritisch`, medium && `${medium} wichtig`, low && `${low} Hinweis${low === 1 ? '' : 'e'}`].filter(Boolean);
  return h('span', { class: high ? 'red' : medium ? 'orange' : parts.length ? null : 'green', title: 'Ergebnis der letzten Sicherheitsprüfung' },
    `Sicherheit: ${parts.length ? parts.join(', ') : 'kein Befund'}`);
}

/** The fixes the report offers, each once (with the first check that offers it). */
function securityOffers(report) {
  const offers = new Map();
  for (const c of report?.checks ?? []) if (c.fix && !offers.has(c.fix.fix)) offers.set(c.fix.fix, { ...c.fix, check: c });
  return [...offers.values()];
}

function securityItem(r, c) {
  const [cls, text] = SEC_LEVEL[c.level];
  const locked = r.job ? 'gehört zu einem offenen Auftrag' : null;
  return h('div', { class: 'item' },
    h('div', { class: 'item-line' },
      badge(cls, text), h('strong', { class: 'item-title', title: c.title }, c.title), h('span', { class: 'spacer' }),
      c.fix && button(`${c.fix.label} …`, () => securityFixDialog(r, c.fix.fix), {
        cls: 'link', disabled: !!locked, title: locked ? `Nicht möglich: ${locked}` : c.fix.effect, id: domId('sec-fix', `${r.key}-${c.id}`),
      }),
      c.url && button(`Auf ${serviceOf(r.repo)}`, () => call('open_url', { url: c.url }), { cls: 'link', title: `Einstellung oder Alerts auf ${serviceOf(r.repo)} öffnen` })),
    h('div', { class: 'item-reason selectable' }, c.detail));
}

function securitySection(r) {
  const report = r.security;
  const offers = securityOffers(report);
  const locked = r.job ? 'gehört zu einem offenen Auftrag' : null;
  const head = h('div', { class: 'section-head wrap' }, h('h3', null, 'Sicherheit'),
    report && h('span', { class: 'muted small' }, `geprüft ${ago(report.checkedAt)}`),
    h('span', { class: 'spacer' }),
    offers.length > 0 && button(`Beheben (${offers.length}) …`, () => securityFixDialog(r), {
      id: 'repo-security-fix', iconName: 'shield', disabled: !!locked,
      title: locked ? `Nicht möglich: ${locked}` : `Einstellungen auf ${serviceOf(r.repo)} ändern; du wählst, welche`,
    }),
    button(r.securityLoading ? 'Wird geprüft …' : report ? 'Neu prüfen' : 'Sicherheit prüfen', () => call('check_security', { keys: [r.key] }),
      { id: 'repo-security-check', iconName: report ? 'refresh' : 'shield', disabled: r.securityLoading, title: 'Nur lesend, ohne Claude' }));
  if (!report) {
    return [head,
      h('div', { class: 'note' }, r.securityLoading ? 'Schutzregeln, Einstellungen und Alerts werden gelesen …'
        : gitlabHost(r.repo)
          ? 'Noch nicht geprüft. Gelesen werden Branch-Schutz, Pipeline-Pflicht, Push-Regel gegen Secrets, Secret Detection, Renovate, Job-Token-Zugriff und Deploy-Keys; nur lesend, ohne Claude.'
          : 'Noch nicht geprüft. Gelesen werden Branch-Schutz, Scanning-Einstellungen, offene Alerts und die Rechte des Workflow-Tokens; nur lesend, ohne Claude.'),
      r.securityError && h('div', { class: 'note error-text' }, `Prüfung fehlgeschlagen: ${r.securityError}`)].filter(Boolean);
  }
  const findings = report.checks.filter((c) => FINDING_LEVELS.includes(c.level));
  const rest = report.checks.filter((c) => !FINDING_LEVELS.includes(c.level));
  const okCount = rest.filter((c) => c.level === 'ok').length;
  const unknownCount = rest.length - okCount;
  return [head,
    r.securityError && h('div', { class: 'note orange' }, `Letzte Prüfung fehlgeschlagen, gezeigt wird die ältere: ${r.securityError}`),
    report.archived ? h('div', { class: 'note' }, 'Archiviert: Einstellungen lassen sich nicht ändern.')
      : report.admin !== true && h('div', { class: 'note orange' },
        gitlabHost(r.repo)
          ? (report.admin == null ? `Ohne glab-Anmeldung ist vieles nicht lesbar, und nichts lässt sich in der App beheben (glab auth login --hostname ${gitlabHost(r.repo)}, oder ein eigenes Token in den Einstellungen).`
            : 'Ohne Maintainer-Rechte sind manche Einstellungen nicht lesbar, und nichts lässt sich in der App beheben.')
          : report.admin == null ? 'Ohne gh-Anmeldung ist vieles nicht lesbar, und nichts lässt sich in der App beheben (gh auth login, oder ein eigenes Token in den Einstellungen).'
            : 'Ohne Admin-Rechte sind manche Einstellungen nicht lesbar, und nichts lässt sich in der App beheben.'),
    findings.length ? h('div', { class: 'items' }, findings.map((c) => securityItem(r, c))) : h('div', { class: 'note green' }, 'Kein Befund.'),
    rest.length > 0 && h('details', { class: 'sec-more' },
      h('summary', null, [okCount && `${okCount} in Ordnung`, unknownCount && `${unknownCount} nicht prüfbar`].filter(Boolean).join(', ')),
      h('div', { class: 'items' }, rest.map((c) => securityItem(r, c)))),
  ].filter(Boolean);
}

/** Asks which fixes to carry out (`only`: just that one ticked), then shows what GitHub answered. */
function securityFixDialog(r, only) {
  const offers = securityOffers(r.security);
  // Without `only`, what can break something stays unticked until you tick it.
  const picked = new Set(offers.filter((o) => (only ? o.fix === only : !o.warning)).map((o) => o.fix));
  const s = { busy: false, error: '', results: null };
  const name = (fix) => offers.find((o) => o.fix === fix)?.label ?? fix;
  openDialog('Sicherheit verbessern', (close, render) => {
    if (s.results) {
      const failed = s.results.filter((x) => !x.ok).length;
      return {
        body: [
          h('h2', null, failed ? `${s.results.length - failed} von ${s.results.length} geändert` : 'Geändert'),
          h('div', { class: 'items' }, s.results.map((x) => h('div', { class: 'item' },
            h('div', { class: 'item-line' }, badge(x.ok ? 'green' : 'red', x.ok ? 'Erledigt' : 'Abgelehnt'), h('strong', null, name(x.fix))),
            h('div', { class: 'item-reason selectable' }, x.text)))),
          h('div', { class: 'note' }, 'Die Sicherheit des Repositorys wird jetzt neu geprüft.'),
        ],
        actions: [button('Schließen', close, { cls: 'primary' })],
      };
    }
    return {
      body: [
        h('h2', null, `${r.repo}: Sicherheit verbessern`),
        h('div', { class: 'note' }, `Jeder Punkt ändert eine Einstellung auf ${serviceOf(r.repo)}. Er fügt Schutz hinzu, entfernt nichts und lässt sich in den Einstellungen des Repositorys auf ${serviceOf(r.repo)} wieder zurücknehmen.`),
        h('div', { class: 'sec-choices' }, offers.map((o) => h('label', { class: 'check-line top' },
          h('input', { type: 'checkbox', id: domId('sec-pick', o.fix), checked: picked.has(o.fix), disabled: s.busy, onchange: (e) => { e.target.checked ? picked.add(o.fix) : picked.delete(o.fix); render(); } }),
          h('span', null,
            h('strong', null, o.label), ' ', h('span', { class: 'muted small' }, `· ${o.check.title}`),
            h('div', { class: 'note' }, o.effect),
            o.warning && h('div', { class: 'note orange' }, o.warning))))),
        s.error && h('div', { class: 'note error-text', role: 'alert' }, s.error),
      ],
      actions: [
        button('Abbrechen', close, { disabled: s.busy }),
        button(s.busy ? 'Wird geändert …' : picked.size > 1 ? `${picked.size} Änderungen ausführen` : `Auf ${serviceOf(r.repo)} ändern`, async () => {
          s.busy = true; s.error = ''; render();
          try {
            s.results = await invoke('fix_security', { key: r.key, fixes: [...picked] });
          } catch (error) { s.error = String(error); }
          s.busy = false; render();
        }, { cls: 'primary', iconName: 'shield', disabled: s.busy || !picked.size }),
      ],
    };
  });
}

// MARK: Repositories: content targets

const KIND_TEXT = { docs: 'README/Dokumentation', wiki: 'Wiki' };
const BASIS_TEXT = { release: 'neuestes Release', commit: 'aktueller Stand des Default-Branches' };
const BUMP_TEXT = { patch: 'Patch', minor: 'Minor', major: 'Major' };
const BUMP_NOTE = { patch: 'Korrekturen (1.2.3 → 1.2.4)', minor: 'neue Funktionen (1.2.3 → 1.3.0)', major: 'inkompatible Änderungen (1.2.3 → 2.0.0)' };

/** The version after `tag` for the bump, as `repocare::next_version` computes it; null for a tag that is no plain x.y.z. */
function nextVersion(tag, prefix, bump) {
  if (!tag.startsWith(prefix)) return null;
  const parts = tag.slice(prefix.length).split('.');
  if (parts.length !== 3 || !parts.every((p) => /^\d+$/.test(p))) return null;
  const [major, minor, patch] = parts.map(Number);
  const next = bump === 'major' ? [major + 1, 0, 0] : bump === 'minor' ? [major, minor + 1, 0] : [major, minor, patch + 1];
  return `${prefix}${next.join('.')}`;
}

function deliveryText(d) {
  if (!d) return null;
  const sources = d.sources.map((s) => s.label.includes(' ') ? `${s.repo} @ ${s.label}` : `${s.repo} ${s.label}`).join(', ');
  return `${d.what || 'Vorschlag'} für ${sources} (${ago(d.at)})`;
}

function targetSummary(t) {
  const s = t.state;
  return [
    s.published ? `Veröffentlicht: ${deliveryText(s.published)}` : 'Noch nie über die App veröffentlicht',
    s.delivered && s.delivered.pr != null && `Wartet auf Merge: ${deliveryText(s.delivered)}`,
    s.prepared && (!s.delivered || s.prepared.at > s.delivered.at) && `Zuletzt vorbereitet: ${deliveryText(s.prepared)}`,
  ].filter(Boolean);
}

function targetItem(t, actions = true) {
  return h('div', { class: 'item target-item' },
    h('div', { class: 'item-line' },
      h('strong', { class: 'item-title' }, t.name), badge(t.kind === 'wiki' ? 'accent' : '', KIND_TEXT[t.kind]),
      h('span', { class: 'muted small mono' }, t.repo),
      h('span', { class: 'spacer' }),
      t.job && jobChip(t.job.status, false),
      actions && button('Bearbeiten …', () => targetDialog(t), { cls: 'link', disabled: !!t.job, title: t.job ? 'Gehört zu einem offenen Auftrag' : null }),
      actions && button('Entfernen', () => confirmDialog('Inhaltsziel entfernen', `„${t.name}“ aus der App entfernen? Im Repository und im Wiki ändert sich nichts.`, 'Entfernen',
        () => act('remove_target', { id: t.id })), { cls: 'link danger', disabled: !!t.job })),
    h('div', { class: 'item-reason' }, `Quellen: ${t.sources.map(repoName).join(', ')} · Basis: ${BASIS_TEXT[t.basis]} · Pfade: ${t.paths.join(', ')}${t.kind === 'docs' ? ` · Branch: ${t.branch || 'Default-Branch'}` : ''}${t.mergeWhenGreen ? ' · mergt bei grüner CI' : ''}`),
    targetSummary(t).map((line) => h('div', { class: 'item-reason' }, line)),
    t.freshness.map((line) => h('div', { class: 'item-reason orange' }, line)),
    t.problem && h('div', { class: 'note error-text' }, `Nicht nutzbar: ${t.problem}`));
}

function contentSection(r) {
  const targets = view.targets.filter((t) => t.sources.includes(r.key));
  return [
    h('div', { class: 'section-head wrap' }, h('h3', null, 'Zugeordnete Inhalte'),
      h('span', { class: 'muted small' }, 'README und Wiki; jedes Ziel ist für sich aktuell oder nicht'),
      h('span', { class: 'spacer' }),
      button('Ziel hinzufügen …', () => targetDialog(null, r.key), { id: 'target-add', iconName: 'plus' })),
    targets.length ? h('div', { class: 'items' }, targets.map((t) => targetItem(t))) : h('div', { class: 'note' }, 'Noch kein README-, Dokumentations- oder Wiki-Ziel für dieses Repository.'),
  ];
}

// MARK: Repositories: management

// Unsaved edits of the management form, per repository; a re-render (new view) keeps them.
const manageDrafts = new Map();

// MARK: Repositories: analysis and memory

// How far a task of the catalog goes on its own (catalog.rs).
const AUTONOMY = { propose: ['', 'nur vorschlagen'], approve: ['accent', 'mit Freigabe'], auto: ['green', 'automatisch'] };
const NOTE_KIND = { verdict: ['accent', 'Bewertung'], failure: ['red', 'Fehlschlag'], skipped: ['', 'Ausgelassen'] };
const autonomyBadge = (level) => badge(AUTONOMY[level]?.[0] ?? '', AUTONOMY[level]?.[1] ?? level, 'Wie weit die App diese Aufgabe allein ausführt');
/** The catalog's offer `id` for a repository, if any. */
const offerOf = (r, id) => r.offers?.find((o) => o.id === id);

function analysisSummary(r) {
  const p = r.profile;
  const notes = r.memory.length ? ` · ${r.memory.length} ${r.memory.length === 1 ? 'Notiz' : 'Notizen'}` : '';
  if (!p) return `Analyse und Gedächtnis${notes}`;
  const stacks = p.stacks.map((s) => s.kind[0].toUpperCase() + s.kind.slice(1)).join(', ');
  return `Analyse und Gedächtnis · ${stacks || 'kein Stack erkannt'}${notes}`;
}

function analysisSection(r) {
  const p = r.profile;
  const row = (label, value, cls = '') => [h('span', { class: 'label' }, label), h('span', { class: `detail-text selectable ${cls}` }, value)];
  const list = (items, none) => (items.length ? items.join(', ') : none);
  const updates = p && (p.dependabot ? `Dependabot (${p.dependabot}${p.dependabotEcosystems.length ? `: ${p.dependabotEcosystems.join(', ')}` : ''})`
    : p.renovate ? `Renovate (${p.renovate})` : 'keine Konfiguration erkannt');
  const facts = p
    ? h('div', { class: 'fact-grid' },
      row('Stand', `${short(p.commit)} (${p.rev}), gelesen ${ago(p.readAt)}`),
      row('Stack', list(p.stacks.map((s) => `${s.kind[0].toUpperCase()}${s.kind.slice(1)} (${s.manifests.join(', ')})`), 'nichts erkannt'), p.stacks.length ? '' : 'muted'),
      row('CI', list(p.ci, 'keine Konfiguration erkannt'), p.ci.length ? '' : 'orange'),
      row('Abhängigkeits-Updates', updates, p.dependabot || p.renovate ? '' : 'orange'),
      row('Tests', list(p.tests, 'keine erkannt'), p.tests.length ? '' : 'muted'),
      row('CHANGELOG', p.changelog ?? 'keins im Wurzelordner', p.changelog ? '' : 'muted'),
      row('Versionsdateien', list(p.versionFiles, 'keine erkannt'), p.versionFiles.length ? '' : 'muted'),
      row('Hinweise für Agenten', list([...p.agentDocs, ...p.skills.map((s) => `Skill ${s}`)], 'keine (CLAUDE.md, AGENTS.md, .claude/skills)'), p.agentDocs.length || p.skills.length ? '' : 'muted'),
      p.site && row('Website-Projekt', 'site.json vorhanden; veröffentlicht wird mit OpenSitePilot'))
    : h('div', { class: 'note' }, r.local.state === 'found'
      ? 'Noch keine Analyse: „Status aktualisieren“ liest die lokale Kopie (nur lesend, ohne Claude, ohne Netz).'
      : 'Keine Analyse: sie liest die lokale Kopie, und es gibt keine. Die Aufträge laufen mit den bisherigen Standardwerten.');
  const offers = r.offers.filter((o) => o.blocked || o.autonomy === 'propose' || o.id === 'dependabot-config');
  return [
    h('p', { class: 'note' }, 'Liegt nur im Datenordner der App, nie im Repository. Die Analyse liest den zuletzt geholten Default-Branch der lokalen Kopie; das Gedächtnis füllen die Aufträge und geben es Claude als Material mit, nicht als Anweisung.'),
    facts,
    offers.length > 0 && h('div', { class: 'finding' }, h('strong', { class: 'small' }, 'Aufgaben aus der Analyse'),
      h('div', { class: 'item-list' }, offers.map((o) => h('div', { class: 'item' },
        h('div', { class: 'item-line' }, autonomyBadge(o.autonomy), h('strong', { class: 'item-title' }, o.title)),
        h('div', { class: `item-reason selectable ${o.blocked ? 'orange' : ''}` }, o.blocked ?? o.why))))),
    h('div', { class: 'finding' },
      h('div', { class: 'item-line' }, h('strong', { class: 'small' }, `Gedächtnis (${r.memory.length})`), h('span', { class: 'spacer' }),
        r.memory.length > 0 && button('Alles löschen …', () => confirmDialog('Gedächtnis löschen', `Alle ${r.memory.length} Notizen zu ${r.repo} löschen? Im Repository ändert sich nichts.`, 'Löschen',
          () => act('forget_memory', { key: r.key }), { danger: true }), { cls: 'link', id: domId('memory-clear', r.key) })),
      r.memory.length === 0
        ? h('div', { class: 'note' }, 'Noch nichts gemerkt. Aufträge merken sich hier Claudes Bewertungen, Fehlschläge und was du ausgelassen oder verworfen hast.')
        : h('div', { class: 'item-list' }, [...r.memory].reverse().map((n) => h('div', { class: 'item' },
          h('div', { class: 'item-line' },
            badge(NOTE_KIND[n.kind]?.[0] ?? '', NOTE_KIND[n.kind]?.[1] ?? n.kind), h('strong', { class: 'item-title', title: n.subject }, n.subject),
            n.count > 1 && badge('orange', `${n.count}×`, `in ${n.count} Aufträgen`), h('span', { class: 'spacer' }),
            h('span', { class: 'muted small' }, ago(n.at)),
            iconButton('Notiz löschen', () => act('forget_note', { key: r.key, kind: n.kind, subject: n.subject, at: n.at }), 'trash')),
          h('div', { class: 'item-reason selectable' }, n.text))))),
  ].filter(Boolean);
}

function manageSection(r) {
  const saved = { enabled: r.enabled, localPath: r.local.path ?? '', release: !!r.release, prefix: r.release?.tagPrefix ?? '', publishes: r.release?.publishesRelease ?? false, releasedBy: r.releasedBy ?? '' };
  const s = manageDrafts.get(r.key) ?? { ...saved };
  manageDrafts.set(r.key, s);
  const locked = !!r.job;
  // "Speichern" only matters once something differs from what is saved.
  const changed = () => s.enabled !== saved.enabled || s.localPath.trim() !== saved.localPath || s.release !== saved.release
    || (s.release && (s.prefix.trim() !== saved.prefix || s.publishes !== saved.publishes)) || s.releasedBy !== saved.releasedBy;
  const saveButton = button('Speichern', () => save(), { cls: 'primary', id: 'manage-save' });
  const state = h('span', { class: 'muted small', role: 'status' });
  const update = () => {
    s.dirty = changed();
    saveButton.disabled = locked || !s.dirty;
    state.textContent = s.dirty ? 'Ungespeicherte Änderungen' : '';
  };
  const prefixInput = h('input', { type: 'text', id: 'manage-prefix', value: s.prefix, placeholder: 'leer für 1.2.3, „v“ für v1.2.3', spellcheck: 'false', disabled: !s.release || !!s.releasedBy || locked, oninput: (e) => { s.prefix = e.target.value; update(); } });
  const publishes = h('input', { type: 'checkbox', id: 'manage-publishes', checked: s.publishes, disabled: !s.release || !!s.releasedBy || locked, onchange: (e) => { s.publishes = e.target.checked; update(); } });
  // Repositories whose tag may publish this one: managed, with a release rule, not released by another.
  const sources = view.repositories.filter((o) => o.key !== r.key && o.release && !o.releasedBy && !gitlabHost(o.repo));
  const followers = releasedThrough(r.key);
  const releaseBox = h('input', {
    type: 'checkbox', id: 'manage-release', checked: s.release && !s.releasedBy, disabled: locked || !!s.releasedBy,
    onchange: (e) => { s.release = e.target.checked; prefixInput.disabled = !s.release; publishes.disabled = !s.release; update(); },
  });
  const sourceSelect = h('select', {
    id: 'manage-released-by', 'aria-describedby': 'manage-released-by-note', disabled: locked || followers.length > 0 || !!gitlabHost(r.repo),
    onchange: (e) => {
      s.releasedBy = e.target.value;
      releaseBox.disabled = locked || !!s.releasedBy;
      releaseBox.checked = s.release && !s.releasedBy;
      prefixInput.disabled = !s.release || !!s.releasedBy;
      publishes.disabled = !s.release || !!s.releasedBy;
      update();
    },
  }, h('option', { value: '', selected: !s.releasedBy }, 'niemand: eigene Release-Regel oder keine'),
  sources.map((o) => h('option', { value: o.key, selected: s.releasedBy === o.key }, o.repo)),
  s.releasedBy && !sources.some((o) => o.key === s.releasedBy) && h('option', { value: s.releasedBy, selected: true }, repoName(s.releasedBy)));
  const localInput = h('input', { type: 'text', id: 'manage-local', value: s.localPath, placeholder: 'kein lokaler Klon', spellcheck: 'false', disabled: locked, oninput: (e) => { s.localPath = e.target.value; update(); } });
  const save = async () => {
    const ok = await act('update_repository', {
      key: r.key,
      settings: { enabled: s.enabled, localPath: s.localPath, release: s.release && !s.releasedBy ? { tagPrefix: s.prefix.trim(), publishesRelease: s.publishes } : null, releasedBy: s.releasedBy },
    });
    if (ok) {
      manageDrafts.delete(r.key);
      call('refresh_repos', { keys: [r.key] });
    }
  };
  update();
  return [
    h('div', { class: 'note' }, locked ? 'Gesperrt, solange ein offener Auftrag das Repository hält.' : 'Gilt für alle künftigen Aufträge.'),
    h('label', { class: 'check-line' }, h('input', { type: 'checkbox', id: 'manage-enabled', checked: s.enabled, disabled: locked, onchange: (e) => { s.enabled = e.target.checked; update(); } }),
      'Aktiv: Aufträge dürfen dieses Repository pflegen'),
    h('div', { class: 'field-row' }, h('label', { for: 'manage-local' }, 'Lokale Kopie'),
      h('div', { class: 'line' }, localInput,
        button('Auswählen …', async () => { const p = await call('pick_folder'); if (p) { s.localPath = p; localInput.value = p; update(); } }, { id: 'manage-pick', disabled: locked }),
        !s.localPath && button('Klonen', async () => {
          const path = await call('clone_repo', { repo: r.repo, workspaceDir: '' });
          if (path) { s.localPath = path; localInput.value = path; update(); }
        }, { id: 'manage-clone', disabled: locked, title: `Nach ${view.workspaceDisplay} klonen` }))),
    h('div', { class: 'note' }, 'Optional. Nur fürs Aufräumen lokaler Branches und für Claudes Prüfung im Code.'),
    h('div', { class: 'field-row' }, h('label', { for: 'manage-released-by' }, 'Veröffentlicht durch'), sourceSelect),
    h('div', { class: 'note', id: 'manage-released-by-note' }, followers.length > 0
      ? `Dieses Repository veröffentlicht ${followers.map((f) => f.repo).join(', ')}; es kann darum nicht selbst durch ein anderes veröffentlicht werden.`
      : 'Für ein Repository, dessen Release ein anderes mit demselben Tag erzeugt (z. B. ein öffentlicher Export aus einem privaten Repository): die App taggt hier nie, wartet nach dem Tag dort auf das Release hier und nimmt README und Wiki dieses Repositorys in dieselbe Kette. README und Doku pflegst du dann im Quell-Repository, weil der Export die Dateien hier ersetzt.'),
    h('label', { class: 'check-line top' }, releaseBox, h('span', null, h('strong', null, 'Release-Regel: die App darf die nächste Version taggen'),
      h('div', { class: 'note' }, 'Die App errät keine Versionierung. Getaggt wird nur auf grüner CI genau des Ziel-Commits.'))),
    h('div', { class: 'field-row' }, h('label', { for: 'manage-prefix' }, 'Tag-Präfix'), prefixInput),
    h('label', { class: 'check-line' }, publishes, gitlabHost(r.repo)
      ? 'Eine Pipeline des Projekts veröffentlicht zu jedem Tag ein GitLab-Release; die App wartet darauf'
      : 'Ein Workflow des Repositorys veröffentlicht zu jedem Tag ein GitHub-Release; die App wartet darauf'),
    h('div', { class: 'line' },
      saveButton, state,
      h('span', { class: 'spacer' }),
      button('Aus der Verwaltung entfernen …', () => confirmDialog('Aus der Verwaltung entfernen',
        `${r.repo} aus der Verwaltung nehmen? Gelöscht wird nichts: nicht auf ${serviceOf(r.repo)}, nicht die lokale Kopie.`, 'Entfernen',
        async () => { if (await act('remove_repository', { key: r.key })) { selection.delete(r.key); closeRepo(); } }), { id: 'manage-remove', cls: 'danger', disabled: locked })),
  ].filter(Boolean);
}

// MARK: Runs (care jobs)

// Full jobs as `get_job` returns them, by id; reloaded when the job's `updatedAt` changes.
const jobCache = new Map();
const jobLoading = new Set();
// The user's marks on a plan before approving it, `jobId:planId` → { excluded, included }: items left
// out, and merges chosen among those the plan offers for choice.
const exclusions = new Map();
// Which step blocks the user opened or closed; survives re-rendering.
const stepOpen = new Map();
// What the result panel shows, so an unchanged job is not rebuilt (diffs are large).
let renderedJob = '';

function buildRuns() {
  runsUi = {
    root: h('div', { class: 'area runs-area' }),
    list: h('div', { class: 'runs-list' }),
    detail: h('div', { class: 'job-detail' }),
    status: h('div', { class: 'job-status' }),
    facts: h('div', { class: 'facts' }),
    tabResult: h('button', { type: 'button', class: 'tab', role: 'tab', id: 'job-tab-result', 'aria-controls': 'job-result', onclick: () => showJobTab('result') }, 'Ergebnis'),
    tabLog: h('button', { type: 'button', class: 'tab', role: 'tab', id: 'job-tab-log', 'aria-controls': 'job-log', onclick: () => showJobTab('log') }),
    result: h('div', { class: 'detail-result', id: 'job-result', role: 'tabpanel', 'aria-labelledby': 'job-tab-result' }),
    log: h('pre', { class: 'log', id: 'job-log', role: 'tabpanel', 'aria-labelledby': 'job-tab-log', tabindex: '0' }),
  };
  runsUi.detail.append(runsUi.status, runsUi.facts,
    h('section', { class: 'detail' },
      h('div', { class: 'tabs', role: 'tablist', 'aria-label': 'Auftrag', onkeydown: jobTabKey }, runsUi.tabResult, runsUi.tabLog),
      runsUi.result, runsUi.log));
  runsUi.root.append(runsUi.list, runsUi.detail);
}

function renderRuns() {
  const summary = jobId && view.jobs.find((j) => j.id === jobId);
  if (jobId && !summary) jobId = null;
  runsUi.list.hidden = !!jobId;
  runsUi.detail.hidden = !jobId;
  if (!jobId) {
    headerParts.runs = null;
    renderPageHeader();
    keepFocus(renderJobList);
    return;
  }
  const cached = jobCache.get(jobId);
  if ((!cached || cached.updatedAt !== summary.updatedAt || cached.running !== summary.running) && !jobLoading.has(jobId)) loadJob(jobId);
  keepFocus(() => renderJob(summary, jobCache.get(jobId)));
}

async function loadJob(id) {
  jobLoading.add(id);
  try {
    jobCache.set(id, await invoke('get_job', { id }));
  } catch (error) {
    jobCache.delete(id);
  } finally {
    jobLoading.delete(id);
  }
  if (area === 'runs' && jobId === id) renderRuns();
}

function renderJobList() {
  const open = openJobs();
  const done = view.jobs.filter((j) => JOB_FINISHED.has(j.status.state));
  const row = (j) => h('div', { class: `job-row ${JOB_STATE[j.status.state]?.[0] ?? ''}`, onclick: () => openJob(j.id) },
    h('div', { class: 'job-card-head' },
      h('button', { type: 'button', class: 'repo-name', id: domId('job-row', j.id), onclick: (e) => { e.stopPropagation(); openJob(j.id); } }, h('strong', { class: 'ellipsis' }, jobTitle(j))),
      h('span', { class: 'muted small' }, formatTime(j.createdAt)),
      h('span', { class: 'spacer' }), jobChip(j.status, j.running)),
    h('div', { class: 'job-card-reason selectable' }, j.status.reason),
    stepPills(j, false));
  runsUi.list.replaceChildren(
    h('section', { class: 'section' },
      h('div', { class: 'section-head' }, h('h2', null, 'Offene Aufträge'), h('span', { class: 'muted small' }, 'geplant, in Vorbereitung, mit Freigabe, laufend, wartend oder blockiert')),
      open.length ? h('div', { class: 'job-list' }, open.map(row)) : h('div', { class: 'note' }, 'Keine. Einen Auftrag startest du unter Repositories: Repositories auswählen, dann „Pflegen …“.')),
    h('section', { class: 'section' },
      h('div', { class: 'section-head' }, h('h2', null, 'Bisherige Aufträge'), h('span', { class: 'muted small' }, 'die letzten 50 bleiben erhalten')),
      done.length ? h('div', { class: 'job-list' }, done.map(row)) : h('div', { class: 'note' }, 'Noch keine.')));
}

function openJob(id) {
  const main = document.getElementById('main');
  if (area === 'runs' && !jobId) jobListScroll = main.scrollTop;
  const changedJob = jobId !== id;
  jobId = id;
  renderedJob = '';
  if (changedJob) {
    jobTab = view.jobs.find((j) => j.id === id)?.running ? 'log' : 'result';
    runsUi.log.replaceChildren();
    loadJobLog(id);
  }
  if (area !== 'runs') showArea('runs');
  else renderRuns();
  main.scrollTop = 0;
  document.getElementById('job-back')?.focus({ preventScroll: true });
}

function closeJob() {
  const id = jobId;
  jobId = null;
  renderRuns();
  document.getElementById('main').scrollTop = jobListScroll;
  if (id) document.getElementById(domId('job-row', id))?.focus({ preventScroll: true });
}

async function loadJobLog(id) {
  const lines = await invoke('get_job_log', { id });
  if (jobId !== id) return;
  runsUi.log.replaceChildren();
  appendTo(runsUi.log, lines);
}

function renderJob(summary, job) {
  const status = summary.status;
  const running = summary.running;
  const finished = JOB_FINISHED.has(status.state);
  const canContinue = !running && !finished && ['planned', 'waiting', 'blocked'].includes(status.state);
  const endJob = () => confirmDialog('Auftrag beenden', 'Den Auftrag beenden? Was noch nicht erledigt ist, bleibt unerledigt; nichts Erledigtes wird zurückgenommen.', 'Beenden',
    () => act('end_job', { id: summary.id }));
  const deleteJob = () => confirmDialog('Auftragsverlauf löschen',
    'Gelöscht werden die gespeicherten Daten dieses Auftrags, sein Log und seine temporären Arbeitsdaten. Was der Auftrag auf GitHub oder GitLab oder im Wiki geändert hat, bleibt bestehen und wird nicht zurückgenommen.',
    'Verlauf löschen', () => act('delete_job', { id: summary.id }).then((ok) => ok && closeJob()), { danger: true });
  const more = [
    !running && !finished && ['Auftrag beenden …', endJob, { iconName: 'stop', danger: true }],
    finished && ['Auftragsverlauf löschen …', deleteJob, { iconName: 'trash', danger: true }],
  ].filter(Boolean);
  headerParts.runs = {
    back: ['Alle Aufträge', closeJob, 'job-back'],
    title: `Auftrag: ${jobTitle(summary)}`,
    chip: jobChip(status, running),
    actions: [
      more.length > 0 && menuButton('Weitere Aktionen', () => more, { id: 'job-more' }),
      canContinue && button(status.state === 'waiting' ? 'Erneut prüfen' : 'Fortsetzen', () => act('continue_job', { id: summary.id }),
        { id: 'job-continue', cls: 'primary', iconName: 'play', disabled: !!view.running, title: view.running ? 'Erst nach dem laufenden Schritt' : null }),
      running && button('Abbrechen', () => act('end_job', { id: summary.id }), { id: 'job-cancel', iconName: 'stop', cls: 'danger', title: 'Hält nach dem laufenden Teilschritt an; Erledigtes bleibt' }),
    ],
  };
  renderPageHeader();
  runsUi.status.replaceChildren(
    h('div', { class: `note-box ${JOB_STATE[status.state]?.[0] ?? ''}` },
      h('strong', null, status.reason), status.next && h('div', { class: 'muted small' }, `Nächster Schritt: ${status.next}`)));
  const o = summary.options;
  const gitlabOnly = summary.repos.every((key) => gitlabHost(key));
  const release = o.release && `Release (${BUMP_TEXT[o.bump ?? 'patch']}${o.prepareRelease ? ', mit Versionsdateien und CHANGELOG' : ''})`;
  const activities = [o.prs && (gitlabOnly ? 'Merge Requests' : 'Pull Requests'), o.cleanup && 'Aufräumen', release].filter(Boolean);
  const targetNames = o.targets.map((t) => `${view.targets.find((x) => x.id === t.id)?.name ?? t.id} (${t.basis === 'release' ? 'Release' : 'aktueller Stand'})`);
  runsUi.facts.replaceChildren(...[
    h('span', null, h('strong', null, 'Repositories: '), summary.repos.map(repoName).join(', ')),
    activities.length > 0 && h('span', null, h('strong', null, 'Pflege: '), activities.join(', ')),
    targetNames.length > 0 && h('span', null, h('strong', null, 'Inhalte: '), targetNames.join(', ')),
    o.website && h('span', null, h('strong', null, 'Website: '), 'als letzter Schritt (jetzt in OpenSitePilot)'),
    o.model && h('span', null, h('strong', null, 'Modell: '), modelName(o.model)),
    h('span', null, h('strong', null, 'Begonnen: '), formatTime(summary.createdAt)),
  ].filter(Boolean));
  const key = `${summary.id}:${job?.updatedAt}:${running}:${!!view.running}`;
  if (key !== renderedJob) {
    renderedJob = key;
    runsUi.result.replaceChildren(...(job ? job.steps.map((step) => stepBlock(job, step, running)) : [h('div', { class: 'note' }, 'Wird geladen …')]));
  }
  renderJobTabs(running);
}

function renderJobTabs(running) {
  const log = jobTab === 'log';
  const showsLog = !runsUi.log.hidden;
  for (const [tab, on] of [[runsUi.tabResult, !log], [runsUi.tabLog, log]]) {
    tab.setAttribute('aria-selected', String(on));
    tab.tabIndex = on ? 0 : -1;
  }
  runsUi.tabLog.replaceChildren(running ? h('span', { class: 'spinner' }) : '', 'Log');
  runsUi.result.hidden = log;
  runsUi.log.hidden = !log;
  if (log && !showsLog) runsUi.log.scrollTop = runsUi.log.scrollHeight;
}

function showJobTab(tab) {
  if (jobTab === tab) return;
  jobTab = tab;
  renderJobTabs(view.jobs.find((j) => j.id === jobId)?.running);
}

function jobTabKey(e) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
  e.preventDefault();
  const next = e.key === 'Home' ? 'result' : e.key === 'End' ? 'log' : jobTab === 'log' ? 'result' : 'log';
  showJobTab(next);
  (next === 'log' ? runsUi.tabLog : runsUi.tabResult).focus();
}

// MARK: Runs: steps

function stepBlock(job, step, running) {
  const key = `${job.id}:${step.id}`;
  const decide = step.state === 'needsApproval';
  const open = stepOpen.get(key) ?? (decide || ['failed', 'blocked', 'partial', 'running', 'waiting'].includes(step.state) || job.steps.length === 1);
  const body = {
    care: careBlock, release: releaseBlock, follow: followBlock, docs: contentBlock, wiki: contentBlock, website: websiteBlock, task: taskBlock,
  }[step.kind](job, step, running);
  const stopped = ['failed', 'blocked'].includes(step.state) && step.detail;
  return h('details', { class: `run step-block ${decide ? 'decide' : ''}`, open, ontoggle: (e) => stepOpen.set(key, e.target.open) },
    h('summary', null,
      h('strong', null, step.label), stepChip(job, step),
      h('span', { class: 'muted small ellipsis step-detail', title: step.detail }, step.detail),
      h('span', { class: 'spacer' }),
      step.links.map((l) => button(l.label, () => call('open_url', { url: l.url }), { cls: 'link', iconName: 'external' }))),
    h('div', { class: 'run-body' },
      // The heading cuts the reason short; a stopped step shows it in full, with what to do about it.
      stopped && h('div', { class: `note-box ${step.state === 'failed' ? 'error' : 'warning'}` },
        h('span', { class: 'selectable' }, step.detail),
        claudeLoginHint(step.detail) && h('strong', { class: 'small' }, claudeLoginHint(step.detail))),
      body));
}

/** What a failed Claude run needs from the user, when its message says so (as `claude_login_hint` in jobrun.rs). */
function claudeLoginHint(text) {
  // The care step's own reason already carries the hint.
  return /authenticat|oauth|log ?in\b|\b401\b|invalid api key/i.test(text ?? '') && !(text ?? '').includes('/login')
    ? 'Claude Code ist nicht (mehr) angemeldet: im Terminal claude starten und /login ausführen, dann diesen Schritt neu vorbereiten.'
    : null;
}

function waitsFor(job, step) {
  const pending = step.after.map((id) => job.steps.find((s) => s.id === id)).filter((s) => s && !['done', 'nothingToDo', 'partial', 'rejected'].includes(s.state));
  return pending.length > 0 && step.state === 'pending' && h('div', { class: 'note' }, `Wartet auf: ${pending.map((s) => s.label).join(', ')}`);
}

/** Decision buttons; disabled while anything runs. */
function decisionBar(...buttons) {
  return h('div', { class: 'decision-bar' }, buttons.filter(Boolean),
    view.running && h('span', { class: 'note' }, 'Entscheiden geht, sobald der laufende Schritt fertig ist.'));
}

// What the app checks again right before each kind of write.
const APP_CHECK = {
  merge: 'Direkt davor prüft die App: noch offen, gleicher Commit und Ziel, mergebar, CI des PR-Commits grün; GitHub bekommt den erwarteten Commit mit.',
  close: 'Direkt davor: noch offen, gleicher Commit und Ziel; ein genannter Ersatz-PR muss offen oder gemergt sein. Kommentar und Schließen werden einzeln gemeldet.',
  rebase: 'Direkt davor: noch offen, von Dependabot, gleicher Commit, weiterhin Konflikt oder Rückstand.',
};
const OUTCOMES = {
  done: ['ok', 'Erledigt'], alreadyDone: ['ok', 'Schon erledigt'], skipped: ['warning', 'Übersprungen'], failed: ['error', 'Fehlgeschlagen'],
  cancelled: ['idle', 'Abgebrochen'], excluded: ['idle', 'Ausgenommen'],
};
// What a carried-out item became, per kind, for the repository's summary: "3 gemergt", "2 bereits gelöscht".
const REPAIR_CHECK = 'Anhaken heißt: Claude passt den Code in einer Wegwerf-Kopie auf dem Commit dieses PRs an (Sandbox, Build und Tests müssen grün sein). Die App prüft die geänderten Dateien (keine Pipeline-Dateien, nichts gelöscht) und öffnet daraus einen neuen PR vom Branch openreleasepilot/fix-…; der PR selbst bleibt unberührt. Gemergt wird nichts: Den neuen PR bewertet der nächste Pflegeplan, gemergt wird er erst nach deiner Freigabe. Kostet einen weiteren Claude-Lauf.';
const DONE_TEXT = { repair: 'Reparatur eröffnet', merge: 'gemergt', close: 'geschlossen', rebase: 'Rebase angestoßen', branch: 'gelöscht', localBranch: 'lokal gelöscht', localSync: 'abgeglichen' };

function careBlock(job, step) {
  const set = job.plan;
  if (!set) return [waitsFor(job, step) || h('div', { class: 'note' }, step.state === 'running' ? 'Die App liest die Repositories und lässt Claude die offenen PRs und MRs bewerten …' : 'Noch nicht geprüft.')];
  const decide = step.state === 'needsApproval';
  const markKey = `${job.id}:${set.id}`;
  const marks = exclusions.get(markKey) ?? { excluded: new Set(), included: new Set() };
  exclusions.set(markKey, marks);
  const cost = set.plans.reduce((sum, p) => sum + (p.reviewCost ?? 0), 0);
  const actionable = job.items.filter((i) => (i.optional ? marks.included.has(i.id) : !marks.excluded.has(i.id))).length;
  const offered = job.items.filter((i) => i.optional).length;
  return [
    h('div', { class: 'note' }, `Plan von ${formatTime(set.createdAt)}${cost ? `, Claude-Prüfung ${usd(cost)}` : ''}. ${decide
      ? `Häkchen entfernen nimmt einzelne Aktionen aus${offered ? '; Angebotenes (Merges, von denen Claude abrät, und Reparaturen) läuft nur, wenn du es anhakst' : ''}. Mehr kann die Freigabe nicht hinzufügen. Vor jeder Aktion prüft die App erneut, was sich geändert hat, wird übersprungen.`
      : 'Erledigtes bleibt bestehen, auch nach Fehlern oder Abbruch.'}`),
    h('div', { class: 'runs' }, set.plans.map((p) => planRepo(job, p, decide, marks, set.plans.length === 1))),
    decide && decisionBar(
      button(actionable ? `Plan freigeben (${actionable} Aktion${actionable === 1 ? '' : 'en'})` : 'Freigeben (nichts ausführen)', async () => {
        if (await act('approve_plan', { id: job.id, planId: set.id, excluded: [...marks.excluded], included: [...marks.included] })) exclusions.delete(markKey);
      }, { cls: 'primary', iconName: 'check', disabled: !!view.running, id: 'plan-approve' }),
      button('Plan verwerfen', () => act('reject_plan', { id: job.id, planId: set.id }), { disabled: !!view.running }),
      button('Neu prüfen', () => act('redo_step', { id: job.id, step: 'care' }), { disabled: !!view.running, title: 'Liest die Repositories erneut und lässt Claude neu bewerten (kostet Claude-Nutzung)' })),
    !decide && ['blocked', 'failed'].includes(step.state) && decisionBar(
      button('Neu prüfen', () => act('redo_step', { id: job.id, step: 'care' }), { disabled: !!view.running })),
  ].filter(Boolean);
}

/** Whether an item runs with the marks so far: an offered merge only when chosen, anything else unless left out. */
const runs = (item, marks) => (item.optional ? marks.included.has(item.id) : !marks.excluded.has(item.id));

function itemControl(item, decide, marks, rerender) {
  if (decide) {
    const set = item.optional ? marks.included : marks.excluded;
    return h('input', {
      type: 'checkbox', id: domId('item', item.id), checked: runs(item, marks), 'aria-label': `${item.label} ausführen`,
      onchange: (e) => { (e.target.checked === item.optional) ? set.add(item.id) : set.delete(item.id); rerender(); },
    });
  }
  if (!item.outcome) return item.writing ? h('span', { class: 'chip running' }, h('span', { class: 'spinner' }), 'Läuft') : h('span', { class: 'chip idle' }, 'Offen');
  const [cls, text] = OUTCOMES[item.outcome];
  return h('span', { class: `chip ${cls}` }, text);
}

function planRepo(job, p, decide, marks, single) {
  const items = job.items.filter((i) => i.repo === p.key);
  const rerender = () => { renderedJob = ''; renderRuns(); };
  const line = (item, main, extra = []) => h('div', { class: `item ${decide && !runs(item, marks) ? 'excluded' : ''}` },
    h('div', { class: 'item-line' }, itemControl(item, decide, marks, rerender), main),
    extra, !decide && item.detail && h('div', { class: 'item-reason selectable' }, item.detail));
  const prLine = (item) => {
    const pr = p.prs.find((x) => String(x.number) === item.reference);
    if (!pr) return line(item, h('span', null, item.label));
    const risk = pr.risk && RISKS[pr.risk];
    return line(item, [
      button(requestRef(p.repo, pr.number), () => call('open_url', { url: pr.url }), { cls: 'link mono', title: `Auf ${serviceOf(p.repo)} öffnen` }),
      h('span', { class: 'item-title', title: pr.title }, pr.bump ? `${pr.title.replace(/ from .* to .*$/, '')} ${pr.bump}` : pr.title),
      !pr.dependabot && h('span', { class: 'muted small' }, pr.author),
      pr.fixOf && badge('accent', `Reparatur für ${requestRef(p.repo, pr.fixOf)}`),
      h('span', { class: 'spacer' }), risk && badge(risk[0], `Claude: ${risk[1]}`)], [
      pr.reason && h('div', { class: 'item-reason selectable' }, pr.reviewed ? `Claude: ${pr.reason}` : pr.reason),
      pr.proof && h('div', { class: 'item-reason selectable' }, `Nachweis: ${pr.proof}`),
      item.kind === 'repair' && pr.repair && h('div', { class: 'item-reason selectable' }, `Vorhaben: ${pr.repair}`),
      decide && item.kind === 'repair' && h('div', { class: 'item-check' }, REPAIR_CHECK.replaceAll('PR', requestWord(p.repo))),
      pr.supersededBy && h('div', { class: 'item-reason' }, `Ersetzt durch #${pr.supersededBy}`),
      decide && item.optional && item.kind === 'merge' && pr.fixOf && h('div', { class: 'item-check' }, `Reparatur der App, die sicherheitskritische Dateien ändert: nie von selbst eingeplant. Anhaken heißt: nach der Freigabe mergen, mit denselben Prüfungen direkt davor (noch offen, gleicher Commit, mergebar, CI des ${requestWord(p.repo)}-Commits grün). Den Diff vorher selbst ansehen.`),
      decide && item.optional && item.kind === 'merge' && !pr.fixOf && h('div', { class: 'item-check' }, 'Claude rät, ihn offen zu lassen; nach den Regeln der App wäre er mergebar. Anhaken heißt: nach der Freigabe mergen, mit denselben Prüfungen direkt davor (noch offen, gleicher Commit, mergebar, CI des PR-Commits grün).'),
      decide && APP_CHECK[pr.action] && h('div', { class: 'item-check' }, gitlabHost(p.repo)
        ? APP_CHECK[pr.action].replaceAll('GitHub', 'GitLab').replaceAll('Dependabot', 'Renovate').replaceAll('CI des PR-Commits', 'Pipeline des MR-Commits').replaceAll('PR', 'MR')
        : APP_CHECK[pr.action]),
      decide && pr.action === 'merge' && pr.dependabot && h('div', { class: 'item-check' }, gitlabHost(p.repo)
        ? 'Bedingt: Verursacht ein vorheriger Merge dieses Laufs einen Konflikt, setzt die App statt des Merges das Label „rebase“; Renovate baut den MR dann neu auf.'
        : 'Bedingt: Verursacht ein vorheriger Merge dieses Laufs einen Konflikt, stößt die App statt des Merges @dependabot rebase an.')]);
  };
  const branchLine = (item, list) => {
    const b = list.find((x) => x.name === item.reference);
    return line(item, [h('span', { class: 'mono item-title', title: item.reference }, item.reference), h('span', { class: 'spacer' }), decide && b?.proof.kind === 'afterMerge' && badge('', 'bedingt')],
      decide && b && h('div', { class: 'item-reason' }, `${b.reason}${b.proof.kind === 'afterMerge' ? '; nur wenn dieser Merge gelingt' : ''}. Vorher: gleicher Commit, nicht geschützt, kein offener ${requestWord(p.repo)}, Nachweis gilt bei ${serviceOf(p.repo)} noch.`));
  };
  const group = (title, list) => list.length > 0 && [h('div', { class: 'item-group' }, `${title} (${list.length})`), ...list];
  const of = (kind) => items.filter((i) => i.kind === kind);
  const repairs = items.filter((i) => i.kind === 'repair');
  // A PR offered for repair shows there, with its reason; the rest of the kept ones below.
  const keep = p.prs.filter((pr) => pr.action === 'keep' && !pr.optional && !repairs.some((i) => i.reference === String(pr.number)));
  const choice = items.filter((i) => i.optional && i.kind === 'merge');
  const counts = ['merge', 'close', 'rebase'].map((k) => [k, of(k).filter((i) => !i.optional).length]).filter(([, n]) => n);
  const branches = of('branch').length + of('localBranch').length;
  // Once carried out, the summary tells what happened, not what the plan intended.
  const carried = items.some((i) => i.outcome && i.outcome !== 'excluded');
  const results = () => {
    const counted = items.filter((i) => i.kind !== 'tag' && i.outcome && i.outcome !== 'excluded');
    const n = (test) => counted.filter(test).length;
    const done = Object.entries(DONE_TEXT).map(([kind, text]) => [n((i) => i.kind === kind && i.outcome === 'done'), text, 'green']);
    const gone = n((i) => i.outcome === 'alreadyDone');
    return [...done, [gone, 'bereits gelöscht', ''], [n((i) => i.outcome === 'skipped'), 'übersprungen', 'orange'],
      [n((i) => i.outcome === 'failed'), 'fehlgeschlagen', 'red'], [n((i) => i.outcome === 'cancelled'), 'abgebrochen', '']]
      .filter(([count]) => count > 0).map(([count, text, cls]) => badge(cls, `${count} ${text}`));
  };
  return h('details', { class: 'run repo', open: stepOpen.get(`${job.id}:plan:${p.key}`) ?? (single || !!p.error || decide), ontoggle: (e) => stepOpen.set(`${job.id}:plan:${p.key}`, e.target.open) },
    h('summary', null, h('strong', null, p.name), h('span', { class: 'muted small mono' }, p.repo), h('span', { class: 'spacer' }),
      p.error && badge('red', 'nicht lesbar'),
      carried ? results() : [
        counts.map(([k, n]) => badge(k === 'merge' ? 'green' : k === 'close' ? 'red' : 'accent', `${n} ${{ merge: 'mergen', close: 'schließen', rebase: 'Rebase' }[k]}`)),
        choice.length > 0 && badge('orange', `${choice.length} zur Wahl`),
        repairs.length > 0 && badge('accent', `${repairs.length} ${repairs.length === 1 ? 'Reparatur' : 'Reparaturen'} zur Wahl`),
        of('tag').length > 0 && badge('green', `Tag ${p.tag ?? of('tag')[0].reference} geplant`),
        branches > 0 && badge('', `${branches} Branches löschen`)],
      keep.length > 0 && badge('orange', `${keep.length} ${keep.length === 1 ? 'braucht' : 'brauchen'} dich`)),
    h('div', { class: 'run-body' },
      p.error && h('div', { class: 'note error-text' }, p.error),
      p.reviewNote && h('div', { class: 'note orange' }, p.reviewNote),
      p.proofNote && h('div', { class: 'note' }, p.proofNote),
      h('div', { class: 'items' },
        group('Mergen', of('merge').filter((i) => !i.optional).map(prLine)),
        group('Zur Wahl: Claude rät, offen zu lassen', choice.map(prLine)),
        group('Reparieren lassen', repairs.map(prLine)),
        group('Schließen', of('close').map(prLine)),
        group('Rebase anstoßen', of('rebase').map(prLine)),
        group('Tag', of('tag').map((item) => line(item, [h('strong', null, item.reference), h('span', { class: 'muted small' }, `${BUMP_TEXT[bumpOf(job, p.key)]} nach ${p.lastTag ?? '?'}`), h('span', { class: 'spacer' }), job.options.prepareRelease && badge('accent', 'mit Release-PR'), decide && badge('', 'bedingt')],
          decide && h('div', { class: 'item-check' }, job.options.prepareRelease
            ? `Im Release-Schritt: nur wenn alle geplanten Merges gelingen und ${p.defaultBranch} sich sonst nicht bewegt. Zuerst bringt die App die Versionsdateien und das CHANGELOG auf ${item.reference} und zeigt dir den Diff zur eigenen Freigabe; danach Release-PR, Merge bei grüner CI und der Tag auf dem Merge-Commit, sobald dessen CI grün ist (je bis zu 30 Minuten).`
            : `Im Release-Schritt: nur wenn alle geplanten Merges gelingen, ${p.defaultBranch} sich sonst nicht bewegt und die CI genau dieses Commits grün ist (bis zu 30 Minuten). Direkt vor dem Schreiben prüft die App die Release-Linie erneut.`)))),
        !p.tag && p.tagNote && job.options.release && [h('div', { class: 'item-group' }, 'Tag'), h('div', { class: 'item-reason' }, `Kein Tag: ${p.tagNote}`)],
        group(`Aufräumen auf ${serviceOf(p.repo)}`, of('branch').map((i) => branchLine(i, p.branches))),
        group('Lokale Kopie abgleichen', of('localSync').map((i) => line(i, h('span', null, 'git fetch --prune origin'), decide && h('div', { class: 'item-reason' }, `Vor dem Löschen, weil ${p.localSync}. Misslingt der Abgleich, löscht die App lokal nichts.`)))),
        group('Aufräumen lokal', of('localBranch').map((i) => branchLine(i, p.localBranches))),
        keep.length > 0 && [h('div', { class: 'item-group' }, `Bleibt offen (${keep.length})`), ...keep.map((pr) => h('div', { class: 'item' },
          h('div', { class: 'item-line' }, button(requestRef(p.repo, pr.number), () => call('open_url', { url: pr.url }), { cls: 'link mono' }), h('span', { class: 'item-title' }, pr.title), pr.fixOf && badge('accent', `Reparatur für ${requestRef(p.repo, pr.fixOf)}`), h('span', { class: 'spacer' }), pr.risk && badge(RISKS[pr.risk][0], `Claude: ${RISKS[pr.risk][1]}`)),
          h('div', { class: 'item-reason selectable' }, pr.reviewed ? `Claude: ${pr.reason}` : pr.reason),
          pr.proof && h('div', { class: 'item-reason selectable' }, `Nachweis: ${pr.proof}`)))]),
      p.cleanupNote && h('div', { class: 'note orange' }, p.cleanupNote),
      p.cloneNote && h('div', { class: 'note orange' }, `Lokale Branches übersprungen: ${p.cloneNote}`),
      p.githubDeletesMerged && of('merge').length > 0 && h('div', { class: 'note' }, 'GitHub löscht die Branches gemergter PRs in diesem Repository selbst.'),
      !p.error && !items.length && !keep.length && h('div', { class: 'note' }, 'Nichts zu tun.')));
}

/** Waiting for the release another repository's tag publishes here; writes nothing. */
function followBlock(job, step) {
  const upstream = step.after.map((id) => job.steps.find((s) => s.id === id)).find((s) => s?.kind === 'release');
  return [
    waitsFor(job, step),
    h('div', { class: 'facts' },
      h('span', null, h('strong', null, 'Repository: '), repoName(step.subject)),
      upstream && h('span', null, h('strong', null, 'Veröffentlicht durch: '), repoName(upstream.subject)),
      h('span', null, h('strong', null, 'Tag: '), upstream?.tag ? `${upstream.tag} (aus ${repoName(upstream.subject)})` : 'erst nach dem Tag dort'),
      h('span', null, h('strong', null, 'Die App: '), 'taggt hier nicht, wartet auf das veröffentlichte Release (bis zu 30 Minuten je Prüfung) und meldet einen fehlgeschlagenen Export- oder Release-Workflow')),
  ];
}

function releaseBlock(job, step) {
  const plan = job.plan?.plans.find((p) => p.key === step.subject);
  const rule = view.repositories.find((r) => r.key === step.subject)?.release;
  const p = step.content;
  const decide = step.state === 'needsApproval';
  const word = requestWord(step.subject);
  const redoable = p && ['needsApproval', 'blocked', 'failed'].includes(step.state) && !step.pr && !step.writing && !step.prepared;
  const failedChecks = p?.checks.filter((c) => !c.ok) ?? [];
  return [
    waitsFor(job, step),
    h('div', { class: 'facts' },
      h('span', null, h('strong', null, 'Repository: '), repoName(step.subject)),
      plan?.tag && h('span', null, h('strong', null, 'Geplant: '), `${plan.tag} (${BUMP_TEXT[bumpOf(job, step.subject)]} nach ${plan.lastTag})`),
      job.options.prepareRelease && h('span', null, h('strong', null, 'Vorher: '), step.prepared ? (step.pr ? `Release-${word} gemergt` : 'nichts an den Versionsdateien zu ändern') : `Release-${word} mit Versionsdateien und CHANGELOG`),
      step.tag && h('span', null, h('strong', null, 'Gesetzt: '), `${step.tag} auf ${short(step.commit)}`),
      rule && h('span', null, h('strong', null, `${serviceOf(step.subject)}-Release: `), rule.publishesRelease ? 'die App wartet darauf (bis zu 30 Minuten je Prüfung)' : 'laut Regel keins')),
    p && decisionBrief([
      ['Version', `${plan?.lastTag ?? '?'} → ${plan?.tag ?? '?'}`],
      ['Änderung', `${p.files.length} ${p.files.length === 1 ? 'Datei' : 'Dateien'}: ${p.files.slice(0, 4).map((f) => f.path).join(', ')}${p.files.length > 4 ? ` und ${p.files.length - 4} weitere` : ''}`],
      ['Ausgangsstand', `${p.baseBranch} ${short(p.baseCommit)} (nach den Merges dieses Auftrags)`],
      ['Prüfungen der App', failedChecks.length ? h('span', { class: 'red' }, `${failedChecks.length} von ${p.checks.length} nicht bestanden`) : `alle ${p.checks.length} bestanden`],
      decide && ['Freigeben bewirkt', `Ein neuer Branch ${p.branch} und ein Release-${word} mit genau diesem Stand. Bei grüner CI mergt die App ihn und setzt den Tag ${plan?.tag ?? ''} auf den Merge-Commit, sobald dessen CI grün ist. Hat sich ${p.baseBranch} inzwischen bewegt, stoppt die App.`],
    ], null),
    p && h('div', { class: 'brief-summary' }, markdown(p.summary)),
    decide && decisionBar(
      button(`Freigeben: Release-${word} erstellen, mergen und taggen`, () => act('approve_step', { id: job.id, step: step.id, fingerprint: p.fingerprint }), { cls: 'primary', iconName: 'check', disabled: !!view.running }),
      button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running, title: `Kein Release-${word}, kein Tag` }),
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running, title: 'Liest den Stand neu und bereitet die Dateien erneut vor' })),
    !decide && redoable && decisionBar(
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running }),
      button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running })),
    step.state === 'waiting' && h('div', { class: 'note orange' }, `${step.detail}. „Erneut prüfen“ oben fragt ${serviceOf(step.subject)} wieder; abhängige Schritte warten so lange.`),
    p && h('details', { class: 'fold' }, h('summary', null, 'Prüfungen im Einzelnen'), checksView(p.checks)),
    p && technicalPreview(`${job.id}:${step.id}:tech`, { files: p.files, diff: p.diff }),
  ].filter(Boolean);
}

function proposalFacts(p) {
  return h('div', { class: 'facts' },
    h('span', null, h('strong', null, 'Quellen: '), p.sources.map((s) => `${s.label.includes(' ') ? `${s.repo} @ ${s.label}` : `${s.repo} ${s.label}`} (${short(s.rev)})`).join(', ')),
    h('span', null, h('strong', null, 'Ausgangsstand: '), `${p.baseBranch} ${short(p.baseCommit)}`),
    p.branch && h('span', null, h('strong', null, 'Neuer Branch: '), h('span', { class: 'mono' }, p.branch)),
    p.costUsd != null && h('span', null, h('strong', null, 'Claude: '), usd(p.costUsd)));
}

function checksView(checks) {
  return h('div', { class: 'checks' }, checks.map((c) => [
    h('span', { class: `status-dot ${c.ok ? 'ok' : 'error'}` }), h('span', { class: 'label' }, c.name), h('span', { class: 'detail-text selectable' }, c.detail)]));
}

function filesView(files) {
  return h('ul', { class: 'plain-list' }, files.map((f) => h('li', null, h('span', { class: 'mono' }, f.path), ' ',
    h('span', { class: 'muted small' }, [f.added ? 'neu' : 'geändert', f.binary ? 'Binärdatei' : null, `${Math.max(1, Math.round(f.size / 1024))} KB`].filter(Boolean).join(', ')))));
}

function diffView(text) {
  const pre = h('pre', { class: 'diff selectable', tabindex: '0', 'aria-label': 'Änderungen' });
  const lines = text.split('\n');
  const parts = lines.map((line) => {
    const cls = line.startsWith('diff --git') ? 'd-head' : line.startsWith('+++') || line.startsWith('---') ? 'd-file'
      : line.startsWith('@@') ? 'd-hunk' : line.startsWith('+') ? 'd-add' : line.startsWith('-') ? 'd-del' : '';
    return h('span', { class: cls || null }, `${line}\n`);
  });
  pre.append(...parts);
  return pre;
}

/** The tail line the app adds to a diff it cut short, e.g. "… gekürzt; vollständig: git -C … show abc1234". */
function diffCut(diff) {
  const at = diff.lastIndexOf('\n… gekürzt');
  return at < 0 ? null : diff.slice(at + 1).trim();
}

/** The technical side of a proposal, after the decision: files, statistics, the diff and its limits. */
function technicalPreview(key, { stat, files, diff }) {
  const cut = diffCut(diff);
  return h('details', { class: 'run tech', open: stepOpen.get(key) ?? true, ontoggle: (e) => stepOpen.set(key, e.target.open) },
    h('summary', null, h('strong', null, 'Technische Änderungsvorschau'), cut && badge('orange', 'gekürzt')),
    h('div', { class: 'run-body' },
      files && h('div', { class: 'section' }, h('strong', { class: 'small' }, `Geänderte Dateien (${files.length})`), filesView(files)),
      stat && h('details', { class: 'fold' }, h('summary', null, 'Git-Statistik'), h('pre', { class: 'result-text mono' }, stat)),
      cut && h('div', { class: 'note orange' }, `Die Vorschau ist gekürzt. Vollständig lokal: ${cut.replace(/^… gekürzt; vollständig: /, '')}`),
      diffView(diff)));
}

/** What a decision is about, before any technical detail: the subject, the change, the effect of approving. */
function decisionBrief(rows, summary) {
  return h('div', { class: 'brief' },
    h('dl', { class: 'brief-facts' }, rows.filter(Boolean).map(([label, value]) => [h('dt', null, label), h('dd', { class: 'selectable' }, value)])),
    summary && h('div', { class: 'brief-summary' }, h('strong', { class: 'small' }, 'Claudes Zusammenfassung'), markdown(summary)));
}

function contentBlock(job, step) {
  const target = view.targets.find((t) => t.id === step.subject);
  const p = step.content;
  const decide = step.state === 'needsApproval';
  const redoable = ['needsApproval', 'blocked', 'failed', 'nothingToDo'].includes(step.state) && !step.pr && !step.writing;
  const docs = step.kind === 'docs';
  const effect = docs
    ? `Ein neuer Branch und ein ${gitlabHost(target?.repo) ? 'Merge Request' : 'Pull Request'} mit genau diesem Stand${target?.mergeWhenGreen ? '; bei grüner CI mergt die App ihn' : '; gemergt wird nicht automatisch'}. Ist die Datei auf dem Zielbranch inzwischen geändert, stoppt die App.`
    : 'Genau dieser Stand wird per Push ohne Force ins Wiki veröffentlicht. Hat jemand das Wiki inzwischen geändert, stoppt die App und überschreibt nichts.';
  const failedChecks = p?.checks.filter((c) => !c.ok) ?? [];
  return [
    waitsFor(job, step),
    !p && h('div', { class: 'facts' },
      h('span', null, h('strong', null, 'Ziel: '), target ? `${target.name} (${KIND_TEXT[target.kind]}) in ${target.repo}` : `${step.subject} (gelöscht)`),
      h('span', null, h('strong', null, 'Basis: '), BASIS_TEXT[step.basis ?? target?.basis ?? 'commit'])),
    p && decisionBrief([
      ['Ziel', target ? `${target.name} (${KIND_TEXT[target.kind]}) in ${target.repo}` : `${step.subject} (gelöscht)`],
      ['Auf Grundlage von', `${p.sources.map((s) => (s.label.includes(' ') ? `${s.repo} @ ${s.label}` : `${s.repo} ${s.label}`)).join(', ')} (${BASIS_TEXT[step.basis ?? target?.basis ?? 'commit']})`],
      ['Änderung', `${p.files.length} ${p.files.length === 1 ? 'Datei' : 'Dateien'}: ${p.files.slice(0, 4).map((f) => f.path).join(', ')}${p.files.length > 4 ? ` und ${p.files.length - 4} weitere` : ''}`],
      ['Prüfungen der App', failedChecks.length ? h('span', { class: 'red' }, `${failedChecks.length} von ${p.checks.length} nicht bestanden`) : `alle ${p.checks.length} bestanden`],
      decide && ['Freigeben bewirkt', effect],
    ], p.summary),
    decide && decisionBar(
      button(docs ? (target?.mergeWhenGreen ? `Freigeben: ${requestWord(target?.repo)} erstellen und bei grüner CI mergen` : `Freigeben: ${requestsTitle(target?.repo).replace(/s$/, '')} erstellen`) : 'Freigeben: im Wiki veröffentlichen',
        () => act('approve_step', { id: job.id, step: step.id, fingerprint: p.fingerprint }), { cls: 'primary', iconName: 'check', disabled: !!view.running }),
      button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running }),
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running, title: 'Neue Quellstände, neuer Claude-Lauf (kostet Claude-Nutzung)' })),
    !decide && redoable && decisionBar(
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running }),
      step.state !== 'nothingToDo' && button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running })),
    p && h('details', { class: 'fold' }, h('summary', null, 'Prüfungen und Stände im Einzelnen'),
      proposalFacts(p), checksView(p.checks)),
    p && technicalPreview(`${job.id}:${step.id}:tech`, { files: p.files, diff: p.diff }),
  ].filter(Boolean);
}

/** A task of the catalog (e.g. a Dependabot configuration): the app's own file, shown with its diff, approved, then a pull request the user merges. */
function taskBlock(job, step) {
  const p = step.content;
  const decide = step.state === 'needsApproval';
  const word = requestWord(step.subject);
  const redoable = ['needsApproval', 'blocked', 'failed', 'nothingToDo'].includes(step.state) && !step.pr && !step.writing;
  const failedChecks = p?.checks.filter((c) => !c.ok) ?? [];
  return [
    waitsFor(job, step),
    !p && h('div', { class: 'facts' }, h('span', null, h('strong', null, 'Repository: '), repoName(step.subject)),
      h('span', null, h('strong', null, 'Freigabe: '), autonomyBadge('approve'))),
    p && decisionBrief([
      ['Repository', repoName(step.subject)],
      ['Änderung', `${p.files.length} ${p.files.length === 1 ? 'Datei' : 'Dateien'}: ${p.files.map((f) => f.path).join(', ')}`],
      ['Ausgangsstand', `${p.baseBranch} ${short(p.baseCommit)}`],
      ['Prüfungen der App', failedChecks.length ? h('span', { class: 'red' }, `${failedChecks.length} von ${p.checks.length} nicht bestanden`) : `alle ${p.checks.length} bestanden`],
      decide && ['Freigeben bewirkt', `Ein neuer Branch ${p.branch} und ein ${word} mit genau diesem Stand. Gemergt wird nicht automatisch; das tust du auf ${serviceOf(step.subject)}. Hat jemand die Datei inzwischen angelegt, stoppt die App.`],
    ], null),
    p && h('div', { class: 'brief-summary' }, markdown(p.summary)),
    decide && decisionBar(
      button(`Freigeben: ${word} erstellen`, () => act('approve_step', { id: job.id, step: step.id, fingerprint: p.fingerprint }), { cls: 'primary', iconName: 'check', disabled: !!view.running }),
      button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running, title: 'Kein PR; der nächste Auftrag weiß, dass du es nicht wolltest' }),
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running, title: 'Liest den Default-Branch neu und erzeugt die Datei erneut (ohne Claude)' })),
    !decide && redoable && decisionBar(
      button('Neu vorbereiten', () => act('redo_step', { id: job.id, step: step.id }), { disabled: !!view.running }),
      step.state !== 'nothingToDo' && button('Verwerfen', () => act('reject_step', { id: job.id, step: step.id }), { disabled: !!view.running })),
    p && h('details', { class: 'fold' }, h('summary', null, 'Prüfungen im Einzelnen'), checksView(p.checks)),
    p && technicalPreview(`${job.id}:${step.id}:tech`, { files: p.files, diff: p.diff }),
  ].filter(Boolean);
}

/** A website step of a job from before OpenSitePilot; it is not run here any more. */
function websiteBlock(job, step) {
  return [h('div', { class: 'note' }, step.detail || 'Die Website aktualisiert und veröffentlicht jetzt OpenSitePilot.')];
}


// MARK: Adding repositories

/** Picks repositories from the local clones in the repo folder (or another folder), or by name for
 *  one without a clone, and adds them to the managed list. */
function addReposDialog() {
  const picked = new Map();
  const s = { dir: '', data: null, error: '', query: '', manual: '', busy: false };
  const list = h('div', { class: 'repo-list', role: 'listbox', 'aria-multiselectable': 'true', 'aria-label': 'Lokale Repositories' });
  // Built once and updated with the list, so filtering never rebuilds the dialog.
  const bulk = h('div', { class: 'line wrap' });
  let render = () => {};
  const manual = () => s.manual.split(/[\s,]+/).filter(Boolean);
  const count = () => picked.size + manual().length;
  const updateButton = () => {
    const b = document.getElementById('add-confirm');
    if (b) { b.disabled = s.busy || !count(); b.textContent = count() ? `Aufnehmen (${count()})` : 'Aufnehmen'; }
  };
  const tag = (text, cls = '') => h('span', { class: `badge ${cls}` }, text);
  const matches = (r) => !s.query || r.repo.toLowerCase().includes(s.query) || r.path.toLowerCase().includes(s.query);
  const renderBulk = () => {
    if (!s.data) { bulk.replaceChildren(); return; }
    // Bulk choices act on the visible matches only; a choice hidden by the filter stays as it is.
    const free = s.data.repos.filter((r) => !r.managed && matches(r));
    const hidden = [...picked.keys()].filter((repo) => !free.some((r) => r.repo === repo)).length;
    const filtered = !!s.query;
    bulk.replaceChildren(
      h('span', { class: 'spacer muted small', role: 'status' },
        [`${picked.size} ausgewählt${hidden && filtered ? `, davon ${hidden} ausgeblendet` : ''}`,
          filtered && `${s.data.repos.filter(matches).length} von ${s.data.repos.length} angezeigt`].filter(Boolean).join(' · ')),
      button(filtered ? 'Alle Treffer auswählen' : 'Alle auswählen', () => { for (const r of free) picked.set(r.repo, r.path); renderList(); },
        { id: 'add-all', disabled: !free.length || free.every((r) => picked.has(r.repo)) }),
      button(filtered ? 'Treffer abwählen' : 'Alle abwählen', () => { for (const r of free) picked.delete(r.repo); renderList(); },
        { id: 'add-none', disabled: !free.some((r) => picked.has(r.repo)) }));
  };
  const renderList = () => {
    const focusId = list.contains(document.activeElement) || bulk.contains(document.activeElement) ? document.activeElement.id : null;
    renderBulk();
    updateButton();
    if (!s.data) { list.replaceChildren(h('div', { class: 'note' }, 'Lokale Repositories werden gesucht …')); return; }
    const repos = s.data.repos.filter(matches);
    list.replaceChildren(...(repos.length ? repos.map((r) => {
      const on = picked.has(r.repo);
      return h('button', {
        type: 'button', role: 'option', id: domId('add-row', r.repo), 'aria-selected': String(on || r.managed), class: `repo-row ${on ? 'picked' : ''}`, disabled: r.managed,
        title: r.managed ? 'wird schon verwaltet' : null,
        onclick: () => { on ? picked.delete(r.repo) : picked.set(r.repo, r.path); renderList(); },
      },
        h('div', { class: 'repo-head' },
          h('input', { type: 'checkbox', checked: on || r.managed, disabled: r.managed, tabindex: '-1', 'aria-hidden': 'true' }),
          h('strong', null, repoParts(r.repo).path),
          gitlabHost(r.repo) && tag(`GitLab · ${gitlabHost(r.repo)}`, 'orange'),
          r.managed && tag('verwaltet', 'green'),
          r.isWebsite && tag('Website-Projekt', 'accent')),
        h('div', { class: 'muted small mono' }, r.path),
        r.others.length > 0 && h('div', { class: 'muted small' }, `Weitere Klone: ${r.others.join(', ')} (in der Verwaltung änderbar)`));
    }) : [h('div', { class: 'note' }, s.data.repos.length ? 'Keine passenden Repositories.' : 'In diesem Ordner liegt kein Git-Klon mit GitHub- oder GitLab-Origin.')]));
    if (focusId) {
      // A bulk button that has just become pointless is disabled; the other one takes the focus.
      const again = document.getElementById(focusId);
      (again && !again.disabled ? again : bulk.querySelector('button:not(:disabled)') ?? search).focus({ preventScroll: true });
    }
  };
  const load = async () => {
    s.data = null;
    renderList();
    s.data = await invoke('list_local_repos', { dir: s.dir || null });
    renderList();
    render();
  };
  const search = h('input', { type: 'search', id: 'add-search', placeholder: 'Filtern …', spellcheck: 'false', autocomplete: 'off', 'aria-label': 'Repositories filtern',
    oninput: (e) => { s.query = e.target.value.trim().toLowerCase(); renderList(); } });
  openDialog('Repositories auswählen', (close, rerender) => {
    render = rerender;
    return {
      body: [
        h('h2', null, 'Repositories auswählen'),
        h('p', { class: 'note', style: 'margin:0' }, 'Die lokalen Git-Klone mit GitHub- oder GitLab-Origin; die lokale Kopie wird gleich mit eingetragen. Nichts wird geklont oder geändert.'),
        h('div', { class: 'line' },
          h('span', { class: 'spacer muted small' }, s.data ? `Ordner: ${s.data.folder} · ${s.data.repos.length} Repositories` : ''),
          button('Anderer Ordner …', async () => { const p = await call('pick_folder'); if (p) { s.dir = p; picked.clear(); load(); } }, { id: 'add-folder' }),
          button('Neu einlesen', load, { id: 'add-reload', iconName: 'refresh', iconOnly: true })),
        h('div', { class: 'repo-picker' }, search, bulk, list),
        h('div', { class: 'field-col' }, h('label', { for: 'add-manual' }, 'Ohne lokale Kopie: Name oder Adresse eingeben (mehrere durch Leerzeichen getrennt)'),
          h('input', { type: 'text', id: 'add-manual', value: s.manual, spellcheck: 'false', autocomplete: 'off', placeholder: 'owner/name oder gitlab.example.com/gruppe/projekt', 'aria-describedby': 'add-manual-note',
            oninput: (e) => { s.manual = e.target.value; updateButton(); } }),
          h('div', { class: 'note', id: 'add-manual-note' }, 'GitHub: owner/name. GitLab: host/gruppe/projekt oder die Adresse des Projekts. Status und Pflege gehen auch ohne Klon; das Projekt muss existieren.')),
        s.error && h('div', { class: 'note error-text', role: 'alert' }, s.error),
      ],
      actions: [
        button('Abbrechen', close),
        button(s.busy ? 'Wird aufgenommen …' : count() ? `Aufnehmen (${count()})` : 'Aufnehmen', async () => {
          const entries = [...picked].map(([repo, localPath]) => ({ repo, localPath }))
            .concat(manual().map((repo) => ({ repo, localPath: '' })));
          s.busy = true; s.error = ''; render();
          try {
            await invoke('add_repositories', { repos: entries });
            close();
            if (area !== 'repos') showArea('repos');
          } catch (error) { s.error = String(error); s.busy = false; render(); }
        }, { id: 'add-confirm', cls: 'primary', disabled: s.busy || !count() }),
      ],
    };
  });
  load();
}

// MARK: Content target dialog

// Fields of the target form in the order they appear; the first with an error gets the focus.
const TARGET_FIELDS = ['name', 'sources', 'repo', 'paths'];

/** Which field a message of the app's own check (content::validate) belongs to; null = the form as a whole. */
function targetErrorField(message) {
  if (/^Name fehlt/.test(message)) return 'name';
  if (/Quell/.test(message)) return 'sources';
  if (/Format owner\/name/.test(message)) return 'repo';
  if (/Pfad|„.*“:|ganze Repository/.test(message)) return 'paths';
  return null;
}

/** Creates or edits a README/docs or wiki target. `source`: the repository it is created from. */
function targetDialog(target, source) {
  const managed = view.repositories;
  const t = target ? structuredClone(target) : {
    id: '', kind: 'docs', name: 'README', sources: source ? [source] : [], repo: source ? repoName(source) : '', branch: '', paths: ['README.md'],
    instructions: '', skill: '', model: '', basis: 'release', check: '', mergeWhenGreen: false,
  };
  // `shown`: fields whose own check shows (left once, or after a save); `server`: messages of the app's check.
  const s = { error: '', paths: t.paths.join('\n'), shown: new Set(), server: {}, sourceQuery: '', advanced: !!(t.skill || t.model || t.check) };
  const fieldRefs = {};
  const check = () => ({
    name: !t.name.trim() && 'Name fehlt.',
    sources: !t.sources.length && 'Mindestens ein Quell-Repository wählen.',
    repo: !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(t.repo.trim())
      && (t.repo.trim() ? `„${t.repo}“ ist kein Repository im Format owner/name.` : t.kind === 'docs' ? 'Ziel-Repository wählen.' : 'Repository des Wikis wählen.'),
    paths: !s.paths.split('\n').some((p) => p.trim()) && 'Mindestens einen erlaubten Pfad angeben.',
  });
  // Updates the messages in place, so typing never rebuilds the form.
  const paint = () => {
    const errors = check();
    for (const [field, ref] of Object.entries(fieldRefs)) {
      const text = s.server[field] || (s.shown.has(field) && errors[field]) || '';
      ref.message.textContent = text;
      ref.message.hidden = !text;
      ref.control.setAttribute('aria-invalid', text ? 'true' : 'false');
    }
  };
  const leave = (field) => () => { s.shown.add(field); paint(); };
  const edited = (field) => { delete s.server[field]; paint(); };
  const field = (name, control) => {
    const message = h('div', { class: 'field-error', id: `t-${name}-error`, role: 'alert', hidden: true });
    control.setAttribute('aria-describedby', [control.getAttribute('aria-describedby'), message.id].filter(Boolean).join(' '));
    fieldRefs[name] = { control, message };
    return message;
  };
  const required = (label) => [label, h('span', { class: 'required', 'aria-hidden': 'true' }, ' *')];
  // The sources are a group of checkboxes: the focus goes to the first one.
  const focusField = (name) => {
    const control = fieldRefs[name]?.control;
    (control?.id === 't-sources' ? control.querySelector('input:not([disabled])') ?? control : control)?.focus();
  };
  const { dialog } = openDialog(target ? 'Inhaltsziel bearbeiten' : 'Inhaltsziel hinzufügen', (close, render) => {
    const docs = t.kind === 'docs';
    const setKind = (kind) => {
      if (kind === t.kind) return;
      t.kind = kind;
      if (!target) {
        t.name = kind === 'wiki' ? 'Wiki' : 'README';
        s.paths = kind === 'wiki' ? '*.md' : 'README.md';
        t.mergeWhenGreen = false;
      }
      s.server = {};
      render();
    };
    const nameInput = h('input', { type: 'text', id: 't-name', value: t.name, required: true, spellcheck: 'false', onblur: leave('name'), oninput: (e) => { t.name = e.target.value; edited('name'); } });
    const sourceBox = h('div', { class: 'source-list', id: 't-sources', role: 'group', tabindex: '-1', 'aria-labelledby': 't-sources-label' },
      managed.map((r) => h('label', { class: 'check-line', 'data-name': r.repo.toLowerCase(), hidden: !!s.sourceQuery && !r.repo.toLowerCase().includes(s.sourceQuery) },
        h('input', { type: 'checkbox', id: domId('t-source', r.key), checked: t.sources.includes(r.key), onchange: (e) => {
          t.sources = e.target.checked ? [...t.sources, r.key] : t.sources.filter((k) => k !== r.key);
          s.shown.add('sources');
          edited('sources');
          if (!t.repo && e.target.checked) { t.repo = r.repo; render(); }
        } }), h('span', { class: 'mono' }, r.repo))));
    const repoSelect = h('select', { id: 't-repo', required: true, onblur: leave('repo'), onchange: (e) => { t.repo = e.target.value; s.shown.add('repo'); edited('repo'); } },
      !managed.some((r) => r.repo === t.repo) && t.repo && h('option', { value: t.repo, selected: true }, t.repo),
      h('option', { value: '', selected: !t.repo }, '– wählen –'),
      managed.map((r) => h('option', { value: r.repo, selected: r.repo === t.repo }, r.repo)));
    const pathsInput = h('textarea', { id: 't-paths', rows: 3, value: s.paths, required: true, spellcheck: 'false', 'aria-describedby': 't-paths-note', onblur: leave('paths'), oninput: (e) => { s.paths = e.target.value; edited('paths'); } });
    const body = [
      h('h2', null, target ? `${t.name} bearbeiten` : 'Inhaltsziel hinzufügen'),
      h('p', { class: 'note', style: 'margin:0' }, 'Claude ändert nur in einem Arbeitsordner der App; du siehst jede Änderung vor der Freigabe. Pflichtfelder sind mit * markiert.'),
      h('fieldset', null, h('legend', null, 'Inhalt'),
        h('div', { class: 'choice-row', role: 'radiogroup', 'aria-label': 'Art' },
          h('label', { class: 'check-line top' }, h('input', { type: 'radio', name: 't-kind', id: 't-kind-docs', checked: docs, onchange: () => setKind('docs') }),
            h('span', null, h('strong', null, 'README oder Dokumentation'), h('div', { class: 'note' }, 'Als neuer Branch und Pull bzw. Merge Request, nie direkt auf den Default-Branch.'))),
          h('label', { class: 'check-line top' }, h('input', { type: 'radio', name: 't-kind', id: 't-kind-wiki', checked: !docs, onchange: () => setKind('wiki') }),
            h('span', null, h('strong', null, 'Wiki (GitHub oder GitLab)'), h('div', { class: 'note' }, 'Nach Freigabe ein Push ohne Force, nur wenn niemand das Wiki inzwischen geändert hat.')))),
        h('div', { class: 'field-row' }, h('label', { for: 't-name' }, required('Name')), h('div', { class: 'field-stack' }, nameInput, field('name', nameInput))),
        h('div', { class: 'field-col' }, h('label', { for: 't-instructions' }, 'Auftrag an Claude'),
          h('textarea', { id: 't-instructions', rows: 3, value: t.instructions, placeholder: 'z. B. „Halte Installation, Funktionen und Konfiguration in der README auf dem Stand der Quellen; Stil und Gliederung beibehalten.“',
            oninput: (e) => { t.instructions = e.target.value; } }))),
      h('fieldset', null, h('legend', null, 'Quellen und Ziel'),
        h('div', { class: 'field-col' },
          h('span', { class: 'field-label', id: 't-sources-label' }, required('Quellen: die Repositories, deren Stand beschrieben wird')),
          managed.length > 6 && h('input', { type: 'search', id: 't-source-search', value: s.sourceQuery, placeholder: 'Quellen filtern …', 'aria-label': 'Quellen filtern', spellcheck: 'false', autocomplete: 'off',
            oninput: (e) => {
              s.sourceQuery = e.target.value.trim().toLowerCase();
              for (const item of sourceBox.children) item.hidden = !!s.sourceQuery && !item.dataset.name.includes(s.sourceQuery);
            } }),
          sourceBox, field('sources', sourceBox)),
        h('div', { class: 'field-row' }, h('label', { for: 't-repo' }, required(docs ? 'Ziel-Repository' : 'Wiki von')), h('div', { class: 'field-stack' }, repoSelect, field('repo', repoSelect))),
        docs && row('Zielbranch', h('input', { type: 'text', value: t.branch, placeholder: 'leer = Default-Branch', spellcheck: 'false', oninput: (e) => { t.branch = e.target.value; } }), 't-branch'),
        h('div', { class: 'field-col' }, h('label', { for: 't-paths' }, required('Erlaubte Pfade (einer pro Zeile)')), pathsInput, field('paths', pathsInput),
          h('div', { class: 'note', id: 't-paths-note' }, '„README.md“ eine Datei, „docs/“ alles darunter, „docs/*.md“ Dateien in einem Ordner, „**/*.md“ über Ordner hinweg. .git, .github und Git-Steuerdateien sind nie erlaubt.'))),
      h('fieldset', null, h('legend', null, 'Aktualisierung'),
        row('Basis', h('select', { onchange: (e) => { t.basis = e.target.value; } },
          h('option', { value: 'release', selected: t.basis === 'release' }, 'Neuestes Release der Quellen'),
          h('option', { value: 'commit', selected: t.basis === 'commit' }, 'Aktueller Stand des Default-Branches')), 't-basis'),
        h('div', { class: 'note' }, 'Pro Auftrag änderbar. Mit „aktueller Stand“ geht es auch ohne neues Release.'),
        docs ? h('label', { class: 'check-line top' }, h('input', { type: 'checkbox', id: 't-merge', checked: t.mergeWhenGreen, onchange: (e) => { t.mergeWhenGreen = e.target.checked; } }),
          h('span', null, h('strong', null, 'Nach Freigabe mergen, sobald die CI grün ist'), h('div', { class: 'note' }, 'Sonst bleibt der Pull bzw. Merge Request offen. Ohne CI-Checks wird nicht automatisch gemergt.')))
          : h('div', { class: 'note' }, 'Ein Wiki wird nach deiner Freigabe direkt veröffentlicht; es hat keine Pull oder Merge Requests.'),
        h('div', { class: 'note' }, 'Jede Vorbereitung ist ein Claude-Lauf und kostet Claude-Nutzung; es gilt die Kostengrenze aus den Einstellungen.')),
      h('details', { class: 'fold', open: s.advanced, ontoggle: (e) => { s.advanced = e.target.open; } },
        h('summary', null, 'Erweiterte Optionen', (t.skill || t.model || t.check) ? h('span', { class: 'muted small' }, ' · angepasst') : null),
        h('div', { class: 'fold-body' },
          row('Skill (optional)', h('input', { type: 'text', value: t.skill, placeholder: 'Name eines Claude-Code-Skills', spellcheck: 'false', oninput: (e) => { t.skill = e.target.value; } }), 't-skill'),
          row('Modell', modelSelect(t.model, view.config.claudeModel, (model) => { t.model = model; }, 't-model'), 't-model'),
          row('Prüfbefehl (optional)', h('input', { type: 'text', value: t.check, placeholder: 'z. B. npx --yes markdownlint-cli2 README.md', spellcheck: 'false', oninput: (e) => { t.check = e.target.value; } }), 't-check'),
          h('div', { class: 'note' }, 'Läuft im Arbeitsordner nach Claudes Änderung, ohne deine GitHub- und GitLab-Tokens; Exit-Code 0 heißt bestanden. Eigene Prüfungen der App laufen immer: Pfade, Konfliktmarker, leere Dateien, neu defekte relative Links.'))),
      s.error && h('div', { class: 'note error-text', role: 'alert', id: 't-form-error', tabindex: '-1' }, s.error),
    ];
    queueMicrotask(paint);
    return {
      body,
      actions: [
        button('Abbrechen', close),
        button('Sichern', async () => {
          s.error = '';
          s.server = {};
          for (const f of TARGET_FIELDS) s.shown.add(f);
          const errors = check();
          const first = TARGET_FIELDS.find((f) => errors[f]);
          if (first) {
            render();
            focusField(first);
            return;
          }
          try {
            await invoke('save_target', { target: { ...t, paths: s.paths.split('\n').map((p) => p.trim()).filter(Boolean) } });
            close();
          } catch (error) {
            const message = String(error);
            const at = targetErrorField(message);
            if (at) s.server[at] = message; else s.error = message;
            render();
            if (at) focusField(at); else dialog.querySelector('#t-form-error')?.focus();
          }
        }, { id: 't-save', cls: 'primary' }),
      ],
    };
  });
}

// MARK: Job dialog

/** A care job for exactly these repositories: what to do, which targets, then start. */
function jobDialog(keys) {
  const repos = keys.map((k) => view.repositories.find((r) => r.key === k)).filter(Boolean);
  const all = repos.filter((r) => !r.capabilities.care && !r.job);
  const excluded = repos.filter((r) => r.capabilities.care || r.job);
  const withRule = all.filter((r) => r.release);
  // Repositories the analysis keeps from a tag (no CI): they stay in the job, but get no tag.
  const untaggable = withRule.filter((r) => offerOf(r, 'release')?.blocked);
  const followers = withRule.flatMap((u) => releasedThrough(u.key).filter((f) => f.enabled).map((f) => ({ follower: f, upstream: u })));
  // The analysis sets the defaults where it knows something; otherwise they stay as before.
  const defaultOn = (id, list, fallback) => {
    const offers = list.map((r) => offerOf(r, id)).filter((x) => x && !x.blocked);
    return offers.length ? offers.some((x) => x.defaultOn) : fallback;
  };
  const o = {
    prs: defaultOn('review-prs', all, true), proof: defaultOn('proof', all, true), cleanup: false, release: false, bump: 'patch', bumps: {},
    prepareRelease: defaultOn('prepare-release', withRule, true), targets: new Map(), tasks: new Set(), model: '', instructions: '',
  };
  // Catalog tasks that are steps of their own, and hints the app only gives.
  const tasks = [...new Set(all.flatMap((r) => r.offers.filter((x) => x.id === 'dependabot-config').map((x) => x.id)))].map((id) => {
    const offered = all.map((r) => ({ r, x: offerOf(r, id) })).filter((e) => e.x);
    return { id, title: offered[0].x.title, autonomy: offered[0].x.autonomy, ok: offered.filter((e) => !e.x.blocked), blocked: offered.filter((e) => e.x.blocked) };
  });
  const hints = all.flatMap((r) => r.offers.filter((x) => x.autonomy === 'propose').map((x) => ({ r, x })));
  // The analysis' reason for a default, shown under the option when one repository is chosen.
  const why = (id) => (all.length === 1 && offerOf(all[0], id) && !offerOf(all[0], id).blocked ? ` Analyse: ${offerOf(all[0], id).why}` : '');
  // With a release, a repository whose release a chosen one publishes comes along by itself, not as a care target.
  const cared = () => all.filter((r) => !(o.release && followers.some((f) => f.follower.key === r.key)));
  // What the chain reaches: the cared-for repositories and, with a release, those their tag publishes.
  const reach = () => [...cared().map((r) => r.key), ...(o.release ? followers.map((f) => f.follower.key) : [])];
  const targetsNow = () => view.targets.filter((t) => t.sources.some((k) => reach().includes(k)));
  // An example of what the chosen bump makes of the newest tag, from the first repository whose tag is known.
  const example = () => withRule.map((r) => {
    // The newest release names the tag; without one, the highest matching tag.
    const last = (r.facts && (ok(r.facts.release) || ok(r.facts.tag))) || null;
    const next = last && nextVersion(last, r.release.tagPrefix, o.bumps[r.key] ?? o.bump);
    return next && `${r.repo}: ${last} → ${next}`;
  }).find(Boolean);
  const s = { error: '', busy: false };
  // The words of the services the chosen repositories live on.
  const lab = all.some((r) => gitlabHost(r.repo));
  const hub = all.some((r) => !gitlabHost(r.repo));
  const pick = (github, gitlab) => (lab && hub ? `${github} bzw. ${gitlab}` : lab ? gitlab : github);
  const service = pick('GitHub', 'GitLab');
  openDialog('Pflegeauftrag', (close, render) => {
    const allowed = cared();
    const targets = targetsNow();
    // A target the chain no longer reaches (release turned off) is not chosen.
    for (const id of [...o.targets.keys()]) if (!targets.some((t) => t.id === id)) o.targets.delete(id);
    const nothing = !o.prs && !o.cleanup && !o.release && !o.targets.size && !o.tasks.size;
    const option = (key, title, note, disabled) => h('label', { class: 'check-line top' },
      h('input', { type: 'checkbox', id: `job-opt-${key}`, checked: o[key], disabled, 'aria-describedby': `job-opt-${key}-note`, onchange: (e) => { o[key] = e.target.checked; render(); } }),
      h('span', null, h('strong', null, title), h('div', { class: 'note', id: `job-opt-${key}-note` }, note)));
    return {
      body: [
        h('h2', null, allowed.length === 1 ? `${allowed[0].repo} pflegen` : `${allowed.length} Repositories pflegen`),
        allowed.length > 1 && h('ul', { class: 'plain-list' }, allowed.map((r) => h('li', { class: 'mono' }, r.repo))),
        excluded.length > 0 && h('div', { class: 'note orange' }, `Ausgenommen: ${excluded.map((r) => `${r.repo} (${r.capabilities.care ?? 'gehört zu einem offenen Auftrag'})`).join(', ')}`),
        h('fieldset', null, h('legend', null, 'Repository-Pflege'),
          option('prs', `${pick('Pull Requests', 'Merge Requests')} prüfen und übernehmen`, `Claude bewertet jeden offenen ${pick('PR', 'MR')} auf den Default-Branch (Risikoklasse, Release-Notes, Versions-Pins, zusammengehörige Abhängigkeiten, Nutzung im Code); das kostet Claude-Nutzung. Gemergt wird nur, was ${service} mergebar mit grüner CI meldet und Claude grün oder gelb einstuft. Überflüssige ${pick('PRs', 'MRs')} werden mit Begründung geschlossen, ${pick('Dependabot-PRs', 'Renovate-MRs')} mit Konflikt neu aufgebaut.${why('review-prs')}`),
          o.prs && h('div', { class: 'sub-option' }, option('proof', 'Mit Nachweisen prüfen', `Statt zu vermuten, baut und testet Claude jeden ${pick('PR', 'MR')} in einer Wegwerf-Kopie und schreibt bei Bedarf einen kurzen Test. In der Sandbox von Claude Code: schreiben nur dort, Netz nur zu Paket-Registries, Zugangsdaten gesperrt. Build-Skripte neuer Abhängigkeiten laufen dabei auf diesem Mac. Dauert länger; ohne lokale Kopie wird nur gelesen; gepusht wird nichts.${why('proof')}`)),
          option('cleanup', 'Gemergte Branches aufräumen', `Branches, deren Commits auf ${service} vollständig im Default-Branch sind oder deren ${pick('PR', 'MR')} gemergt wurde, auf ${service} und in der lokalen Kopie. Geschützte, ausgecheckte und Branches offener ${pick('PRs', 'MRs')} bleiben.`),
          option('release', 'Release: nächste Version taggen', withRule.length
            ? `Nur für Repositories mit Release-Regel (${withRule.filter((r) => !untaggable.includes(r)).map((r) => r.repo).join(', ') || 'keins ohne Hindernis'}), nur auf grüner CI genau des Ziel-Commits; danach wartet die App bei Bedarf auf das veröffentlichte ${service}-Release.`
            : 'Keins der Repositories hat eine Release-Regel; einrichten unter Repository → Verwaltung.', !withRule.length || untaggable.length === withRule.length),
          untaggable.length > 0 && h('div', { class: 'note orange' }, `Kein Tag für ${untaggable.map((r) => `${r.repo} (${offerOf(r, 'release').blocked})`).join(', ')}`),
          o.release && h('div', { class: 'sub-option' },
            h('div', { class: 'field-row' }, h('label', { for: 'job-bump' }, 'Version'),
              h('select', { id: 'job-bump', 'aria-describedby': 'job-bump-note', onchange: (e) => { o.bump = e.target.value; render(); } },
                Object.entries(BUMP_TEXT).map(([value, text]) => h('option', { value, selected: o.bump === value }, `${text}: ${BUMP_NOTE[value]}`)))),
            h('div', { class: 'note', id: 'job-bump-note' }, example() ?? 'Die Version zählt vom höchsten Tag der Release-Regel aus weiter.'),
            withRule.length > 1 && h('div', { class: 'field-col' }, h('div', { class: 'note' }, 'Je Repository abweichend:'),
              withRule.map((r) => h('div', { class: 'field-row' }, h('label', { for: domId('job-bump', r.key), class: 'mono' }, r.repo),
                h('select', { id: domId('job-bump', r.key), onchange: (e) => { if (e.target.value) o.bumps[r.key] = e.target.value; else delete o.bumps[r.key]; render(); } },
                  h('option', { value: '', selected: !o.bumps[r.key] }, `wie oben (${BUMP_TEXT[o.bump]})`),
                  Object.entries(BUMP_TEXT).map(([value, text]) => h('option', { value, selected: o.bumps[r.key] === value }, text)))))),
            followers.length > 0 && h('div', { class: 'note' }, `Kommt mit: ${followers.map((f) => `${f.follower.repo} (Release aus ${f.upstream.repo})`).join(', ')}. Die App taggt dort nicht, sie wartet nach dem Tag auf das Release und meldet einen fehlgeschlagenen Export.`),
            option('prepareRelease', 'Vorher Versionsdateien und CHANGELOG anpassen', `Vor dem Tag bringt die App Cargo.toml, package.json, tauri.conf.json, Chart.yaml und ähnliche Dateien sowie das CHANGELOG.md auf die neue Version, aber nur dort, wo genau die Version des letzten Tags steht. Du siehst den Diff und gibst ihn eigens frei; dann ein Release-${pick('PR', 'MR')}, Merge bei grüner CI und der Tag auf dem Merge-Commit. Ohne passende Datei folgt der Tag direkt.${why('prepare-release')}`))),
        (tasks.length > 0 || hints.length > 0) && h('fieldset', null, h('legend', null, 'Aus der Analyse'),
          tasks.map((t) => h('div', null,
            h('label', { class: 'check-line top' },
              h('input', { type: 'checkbox', id: domId('job-task', t.id), checked: o.tasks.has(t.id), disabled: !t.ok.length, 'aria-describedby': domId('job-task-note', t.id),
                onchange: (e) => { e.target.checked ? o.tasks.add(t.id) : o.tasks.delete(t.id); render(); } }),
              h('span', null, h('strong', null, t.title), ' ', autonomyBadge(t.autonomy),
                h('div', { class: 'note', id: domId('job-task-note', t.id) }, t.ok.length === 1 && all.length === 1 ? t.ok[0].x.why : `${t.ok.map((e) => e.r.repo).join(', ')}: ${t.ok[0]?.x.why ?? ''}`),
                t.blocked.length > 0 && h('div', { class: 'note orange' }, t.blocked.map((e) => `${e.r.repo}: ${e.x.blocked}`).join(' · ')))))),
          hints.map(({ r, x }) => h('div', { class: 'note' }, autonomyBadge(x.autonomy), ' ', h('strong', null, all.length > 1 ? `${r.repo}: ${x.title}` : x.title), ` – ${x.why}`))),
        targets.length > 0 && h('fieldset', null, h('legend', null, 'Zugeordnete Inhalte'),
          targets.map((t) => h('div', { class: 'target-choice' },
            h('label', { class: 'check-line' },
              h('input', { type: 'checkbox', id: domId('job-target', t.id), checked: o.targets.has(t.id), disabled: !!t.problem || !!t.job, onchange: (e) => { e.target.checked ? o.targets.set(t.id, t.basis) : o.targets.delete(t.id); render(); } }),
              h('strong', null, t.name), h('span', { class: 'muted small' }, `${KIND_TEXT[t.kind]} · ${t.repo}`)),
            o.targets.has(t.id) && h('select', { class: 'basis-select', id: domId('job-basis', t.id), 'aria-label': `Basis für ${t.name}`, onchange: (e) => o.targets.set(t.id, e.target.value) },
              h('option', { value: 'release', selected: o.targets.get(t.id) === 'release' }, 'Basis: neuestes Release'),
              h('option', { value: 'commit', selected: o.targets.get(t.id) === 'commit' }, 'Basis: aktueller Stand')),
            (t.problem || t.job) && h('div', { class: 'note orange' }, t.problem ?? 'gehört zu einem offenen Auftrag')))),
        targets.length === 0 && h('div', { class: 'note' }, 'Für README oder Wiki zuerst ein Inhaltsziel anlegen (Repository → Zugeordnete Inhalte).'),
        (o.prs || o.targets.size > 0) && row('Modell', modelSelect(o.model, view.config.claudeModel, (model) => { o.model = model; }, 'job-model'), 'job-model'),
        o.targets.size > 0 && h('div', { class: 'field-col' }, h('label', { for: 'job-instructions' }, 'Zusätzliche Anweisung für Inhalte (optional)'),
          h('textarea', { id: 'job-instructions', rows: 2, value: o.instructions, oninput: (e) => { o.instructions = e.target.value; } })),
        h('div', { class: 'note' }, `Der Auftrag liest und bereitet vor, ohne nachzufragen. Vor jeder Änderung auf ${service} oder im Wiki legt er dir einen Vorschlag zur Freigabe vor; der Fortschritt steht unter Aufträge.`),
        s.error && h('div', { class: 'note error-text', role: 'alert' }, s.error),
      ],
      actions: [
        button('Abbrechen', close),
        button(s.busy ? 'Wird gestartet …' : 'Auftrag starten', async () => {
          s.busy = true; s.error = ''; render();
          try {
            const options = {
              prs: o.prs, proof: o.prs && o.proof, cleanup: o.cleanup, release: o.release, bump: o.bump, bumps: o.release ? o.bumps : {}, prepareRelease: o.release && o.prepareRelease,
              model: o.model, instructions: o.instructions, tasks: [...o.tasks],
              targets: [...o.targets].map(([id, basis]) => ({ id, basis })),
            };
            const id = await invoke('start_job', { input: { repos: allowed.map((r) => r.key), options } });
            for (const r of all) selection.delete(r.key);
            close();
            jobTab = 'log';
            openJob(id);
          } catch (error) { s.error = String(error); s.busy = false; render(); }
        }, { cls: 'primary', iconName: 'play', disabled: s.busy || nothing || !allowed.length || !!view.running }),
      ],
    };
  });
}

// MARK: Settings

/** The cost limit as typed, as the app checks it: empty = none, else a positive amount with point or comma. */
function parseBudget(text) {
  const value = String(text ?? '').trim();
  if (!value) return { ok: true, value: null };
  const amount = /^\d+([.,]\d+)?$/.test(value) ? Number(value.replace(',', '.')) : NaN;
  return Number.isFinite(amount) && amount > 0 ? { ok: true, value: amount }
    : { ok: false, error: 'Bitte einen Betrag größer als 0 eingeben, z. B. 1,50, oder das Feld leeren.' };
}

function settingsDialog() {
  const c = view.config;
  const s = {
    workspaceDir: c.workspaceDir ?? '', message: null,
    updateCheck: !c.noUpdateCheck, claudeModel: c.claudeModel ?? '', budget: c.maxBudgetUsd != null ? String(c.maxBudgetUsd).replace('.', ',') : '',
    open: { general: true, app: false },
  };
  const input = () => ({ workspaceDir: s.workspaceDir, updateCheck: s.updateCheck, claudeModel: s.claudeModel, maxBudgetUsd: String(s.budget ?? '') });
  const text = (key, props = {}, after) => h('input', { type: 'text', value: s[key] ?? '', spellcheck: 'false', autocomplete: 'off', ...props,
    oninput: (e) => { s[key] = e.target.value; after?.(); } });
  let render = () => {};
  // The budget's error is updated in place while typing; the field keeps focus and caret.
  const budgetError = h('div', { class: 'field-error', id: 's-budget-error', role: 'alert', hidden: true });
  const showBudget = (input) => {
    const parsed = parseBudget(s.budget);
    budgetError.textContent = parsed.ok ? '' : parsed.error;
    budgetError.hidden = parsed.ok;
    input?.setAttribute('aria-invalid', parsed.ok ? 'false' : 'true');
    return parsed.ok;
  };
  const section = (key, title, ...content) => h('details', { class: 'fold settings-group', open: s.open[key], ontoggle: (e) => { s.open[key] = e.target.open; } },
    h('summary', null, title), h('div', { class: 'fold-body' }, content));
  // Own tokens: the form for GitHub or one GitLab instance; the token lives only in this field's state until saved.
  const tok = { forge: null, host: '', scheme: 'https', storage: 'keychain', token: '', busy: false, error: null, ok: null, lockHost: false };
  const tokClose = () => { tok.forge = null; tok.token = ''; tok.error = null; tok.busy = false; render(); };
  const storageLabel = (storage) => (storage === 'app' ? 'in der App' : 'Schlüsselbund');
  const tokOpen = (kind, host = '', scheme = 'https', storage = 'keychain') => {
    Object.assign(tok, { forge: kind, host, scheme, storage: storage === 'app' ? 'app' : 'keychain', token: '', error: null, ok: null, busy: false, lockHost: !!host });
    render();
    queueMicrotask(() => document.getElementById(kind === 'gitlab' && !host ? 's-tok-host' : 's-tok-token')?.focus());
  };
  const tokSave = async () => {
    const host = tok.host.trim();
    if (tok.forge === 'gitlab' && !host) { tok.error = 'Bitte die Instanz angeben, z. B. gitlab.example.com.'; render(); return; }
    if (!tok.token.trim()) { tok.error = 'Bitte das Token einfügen.'; render(); return; }
    tok.busy = true; tok.error = null; render();
    try {
      const login = await invoke('save_forge_token', {
        forge: tok.forge === 'github' ? { kind: 'github' } : { kind: 'gitlab', host },
        token: tok.token.trim(), scheme: tok.forge === 'gitlab' ? tok.scheme : null, storage: tok.storage,
      });
      const kind = tok.forge;
      tok.forge = null; tok.token = ''; tok.busy = false;
      tok.ok = { kind, host, text: `Angemeldet als ${login}` };
      await refresh();
    } catch (error) {
      tok.busy = false; tok.token = ''; tok.error = String(error);
    }
    render();
  };
  const tokForget = async (forgeArg) => {
    tok.ok = null;
    if (await act('forget_forge_token', { forge: forgeArg })) await refresh();
    render();
  };
  // The form is a box of its own under the service's status: labels above the fields, so it fits the narrow column.
  const tokField = (label, id, control, hint) => h('div', { class: 'token-field' },
    h('label', { class: 'field-label', for: id }, label), control, hint && h('div', { class: 'note' }, hint));
  const tokStorage = (value, title, hint) => h('label', { class: 'token-choice' },
    h('input', { type: 'radio', name: 's-tok-storage', value, checked: tok.storage === value, disabled: tok.busy, onchange: () => { tok.storage = value; } }),
    h('span', null, h('span', null, title), h('span', { class: 'note' }, hint)));
  const tokForm = (kind) => h('div', { class: 'token-form', role: 'group', 'aria-label': kind === 'github' ? 'Eigenes GitHub-Token' : 'GitLab-Instanz mit Token' },
    kind === 'gitlab' && tokField('Instanz', 's-tok-host',
      h('div', { class: 'token-host' },
        h('select', { id: 's-tok-scheme', 'aria-label': 'Protokoll', disabled: tok.busy, onchange: (e) => { tok.scheme = e.target.value; render(); } },
          h('option', { value: 'https', selected: tok.scheme === 'https' }, 'https://'),
          h('option', { value: 'http', selected: tok.scheme === 'http' }, 'http://')),
        h('input', { id: 's-tok-host', type: 'text', value: tok.host, spellcheck: 'false', autocomplete: 'off', disabled: tok.busy || tok.lockHost,
          placeholder: 'gitlab.example.com', oninput: (e) => { tok.host = e.target.value; } })),
      tok.scheme === 'http' ? 'http ohne TLS nur für eine Instanz im eigenen Netz: Token und Daten gehen unverschlüsselt.' : 'Mit Port, falls nötig, z. B. 192.168.1.5:8929.'),
    tokField('Token', 's-tok-token',
      h('input', { id: 's-tok-token', type: 'password', value: tok.token, spellcheck: 'false', autocomplete: 'off', disabled: tok.busy,
        oninput: (e) => { tok.token = e.target.value; }, onkeydown: (e) => { if (e.key === 'Enter') { e.preventDefault(); tokSave(); } } }),
      kind === 'github'
        ? 'Anlegen auf GitHub unter Settings → Developer settings → Personal access tokens, Scopes „repo“ und „workflow“.'
        : 'Anlegen in GitLab unter Benutzereinstellungen → Zugriffstoken, Scope „api“.'),
    h('div', { class: 'token-field' }, h('span', { class: 'field-label' }, 'Ablage'),
      h('div', { class: 'token-choices' },
        tokStorage('keychain', 'Schlüsselbund (empfohlen)', 'macOS fragt beim ersten Zugriff eventuell nach.'),
        tokStorage('app', 'In der App', 'Als Datei im Datenordner der App, nur für dich lesbar.'))),
    tok.error && h('div', { class: 'note error-text', role: 'alert' }, tok.error),
    h('div', { class: 'line' },
      button(tok.busy ? 'Wird geprüft …' : 'Prüfen und sichern', tokSave, { cls: 'primary', disabled: tok.busy, id: 's-tok-save' }),
      button('Abbrechen', tokClose, { disabled: tok.busy })));
  // One line per account: what signs in, and what can be done about it.
  const topRow = (label, control) => { const r = row(label, control); r.classList.add('top'); return r; };
  const tokLine = (text, cls, ...actions) => h('div', { class: 'line token-line' }, h('span', { class: `spacer ${cls}` }, text), ...actions);
  const githubRow = () => {
    const open = tok.forge === 'github';
    const own = view.config.githubTokenSaved;
    return topRow('GitHub', h('div', { class: 'field-stack' },
      own
        ? tokLine(`Eigenes Token (${storageLabel(view.config.githubTokenStorage)})`, 'muted',
          !open && button('Ersetzen …', () => tokOpen('github', '', 'https', view.config.githubTokenStorage)),
          !open && button('Entfernen', () => tokForget({ kind: 'github' })))
        : tokLine(view.signedIn === false ? 'Nicht angemeldet' : view.signedIn ? 'Über die GitHub-CLI (gh)' : 'Wird beim ersten Lesen geprüft',
          view.signedIn === false ? 'orange' : 'muted',
          !open && button('Eigenes Token …', () => tokOpen('github'))),
      !own && view.signedIn === false && !open && h('div', { class: 'note' }, 'Im Terminal gh auth login, oder ein eigenes Token eintragen.'),
      tok.ok?.kind === 'github' && h('div', { class: 'note ok-text' }, tok.ok.text),
      open && tokForm('github')));
  };
  const gitlabRow = () => {
    const hosts = view.gitlabHosts ?? [];
    const own = view.config.gitlabInstances ?? [];
    const open = tok.forge === 'gitlab';
    const status = (g) => g.ownToken ? `eigenes Token (${storageLabel(g.ownStorage)})`
      : g.signedIn === true ? 'über glab' : g.signedIn === false ? 'nicht angemeldet' : g.configured ? 'in glab eingerichtet' : 'wird beim ersten Lesen geprüft';
    return topRow('GitLab', h('div', { class: 'field-stack' },
      hosts.map((g) => {
        const mine = own.find((i) => i.host === g.host);
        const editing = open && tok.lockHost && tok.host === g.host;
        return tokLine(h('span', { class: 'token-host-status' }, h('span', { class: 'mono' }, g.host),
          h('span', { class: `small ${!g.ownToken && g.signedIn === false ? 'orange' : 'muted'}` }, status(g))), '',
          g.ownToken && !editing && button('Ersetzen …', () => tokOpen('gitlab', g.host, mine?.scheme ?? 'https', mine?.storage ?? g.ownStorage)),
          g.ownToken && !editing && button('Entfernen', () => tokForget({ kind: 'gitlab', host: g.host })),
          !g.ownToken && !open && button('Eigenes Token …', () => tokOpen('gitlab', g.host)));
      }),
      !hosts.length && !open && tokLine('Keine Instanz verbunden', 'muted'),
      (() => {
        const out = hosts.filter((g) => !g.ownToken && g.signedIn === false);
        return out.length > 0 && !open && h('div', { class: 'note' }, `Im Terminal glab auth login --hostname ${out.length === 1 ? out[0].host : '<instanz>'}, oder ein eigenes Token eintragen.`);
      })(),
      tok.ok?.kind === 'gitlab' && h('div', { class: 'note ok-text' }, `${tok.ok.host}: ${tok.ok.text}`),
      open ? tokForm('gitlab') : h('div', { class: 'line' }, button('Instanz hinzufügen …', () => tokOpen('gitlab')))));
  };
  const dlg = openDialog('Einstellungen', (close, rerender) => {
    render = rerender;
    const budgetInput = text('budget', { id: 's-budget', placeholder: 'USD, leer = keine', inputmode: 'decimal', 'aria-describedby': 's-budget-note s-budget-error' }, () => showBudget(budgetInput));
    queueMicrotask(() => { if (s.budget) showBudget(budgetInput); });
    return {
      body: [
        h('h2', null, 'Einstellungen'),
        section('general', 'Allgemein',
          h('fieldset', null, h('legend', null, 'Zugänge'),
            githubRow(),
            gitlabRow(),
            h('div', { class: 'note' }, 'Ohne eigenes Token nutzt die App das Token von gh bzw. glab. Claude bekommt nie eines.'),
            row('Repo-Ordner', h('div', { class: 'line' }, text('workspaceDir', { id: 's-workspace', placeholder: `${view.workspaceDisplay} (automatisch)` }),
              button('Auswählen …', async () => { const p = await call('pick_folder'); if (p) { s.workspaceDir = p; render(); } }, { id: 's-workspace-pick' }))),
            h('div', { class: 'note' }, 'Aus diesem Ordner bietet „Repositories auswählen“ die lokalen Klone an; neue Klone legt die App hier an.')),
          h('fieldset', null, h('legend', null, 'Ausführungsregeln für Aufträge'),
            row('Claude-Modell', modelSelect(s.claudeModel, null, (model) => { s.claudeModel = model; }, 's-claude-model'), 's-claude-model'),
            h('div', { class: 'field-row' }, h('label', { for: 's-budget' }, 'Kostengrenze je Claude-Lauf'), h('div', { class: 'field-stack' }, budgetInput, budgetError)),
            h('div', { class: 'note', id: 's-budget-note' }, 'In USD, Komma oder Punkt. Gilt für PR-Prüfungen und Inhaltsarbeit, wenn Auftrag oder Ziel nichts anderes wählen. Schreibende Schritte laufen nie ohne Vorschau und deine Freigabe.'))),
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
          if (!showBudget(document.getElementById('s-budget'))) {
            s.open.general = true; render();
            document.getElementById('s-budget')?.focus();
            return;
          }
          try {
            await invoke('save_settings', { input: input() });
            close();
          } catch (error) {
            s.message = String(error); render();
            document.getElementById('s-message')?.focus();
          }
        }, { id: 's-save', cls: 'primary' }),
      ],
    };
  });
  return dlg;
}


// MARK: Start

async function start() {
  await listen('changed', refresh);
  await listen('log', (event) => {
    const { job, line } = event.payload;
    if (job && runsUi && job === jobId) appendTo(runsUi.log, [line]);
  });
  // The menu bar icon asks for an area, a repository or a job.
  await listen('show', ({ payload }) => {
    if (!view || !reposUi) return;
    if (payload.repo && view.repositories.some((r) => r.key === payload.repo)) {
      if (area === 'repos') openRepo(payload.repo);
      else { repoKey = payload.repo; showArea('repos'); }
    } else if (payload.job && runsUi && view.jobs.some((j) => j.id === payload.job)) {
      openJob(payload.job);
    } else if (payload.area === 'repos' || payload.area === 'runs') {
      // To the list: showArea closes open details when the area is already shown.
      if (area !== payload.area) { if (payload.area === 'repos') repoKey = null; else jobId = null; }
      showArea(payload.area);
    }
  });
  // A hidden window gets no events (see emit_to_visible); shown again, it fetches what it missed.
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState !== 'visible' || !view) return;
    refresh();
    if (jobId && runsUi) loadJobLog(jobId);
  });
  await refresh();
  // Ages and "veraltet" move on without an event; a fresh view keeps them right. Reads nothing remote.
  setInterval(() => { if (document.visibilityState === 'visible') refresh(); }, 30000);
}

start();
