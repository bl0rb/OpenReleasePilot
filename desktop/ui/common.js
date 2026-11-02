// Shared by both apps' pages: building elements, icons, menus, dialogs, banners, toasts. No framework, no build step.
export const { invoke } = window.__TAURI__.core;
export const { listen } = window.__TAURI__.event;

// Claude Code model aliases; each follows the newest model of its family.
export const MODELS = [['opus', 'Opus'], ['sonnet', 'Sonnet'], ['haiku', 'Haiku'], ['fable', 'Fable']];
export const modelName = (alias) => MODELS.find(([id]) => id === alias)?.[1] ?? alias ?? 'Claude-Code-Standard';

/** Model dropdown; the first entry stands for the default that applies without a choice. */
export function modelSelect(value, fallback, onChange, id) {
  return h('select', { id, class: 'model-select', title: 'Modell für diesen Claude-Lauf', onchange: (e) => onChange(e.target.value) },
    h('option', { value: '', selected: !value }, `Standard (${modelName(fallback)})`),
    MODELS.map(([alias, name]) => h('option', { value: alias, selected: value === alias }, name)));
}

// MARK: Helpers

export const PROPS = new Set(['value', 'checked', 'disabled', 'selected', 'hidden', 'required', 'indeterminate']);

export function h(tag, props, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value == null || value === false) continue;
    if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
    else if (key === 'class') el.className = value;
    else if (PROPS.has(key)) el[key] = value;
    else el.setAttribute(key, value === true ? '' : value);
  }
  for (const child of children.flat(Infinity)) {
    if (child == null || child === false) continue;
    el.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return el;
}

export const ICONS = {
  play: '<path d="M7 5v14l11-7z"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  external: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  forward: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  close: '<path d="M6 6l12 12M18 6 6 18"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  refresh: '<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/>',
  branch: '<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="7" r="2"/><path d="M6 7v10M18 9c0 5-6 4-11 8"/>',
  back: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
  check: '<path d="M5 12l5 5 9-10"/>',
  shield: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/>',
  repo: '<path d="M5 4a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1z"/><path d="M9 3v18M12 8h4M12 12h4"/>',
  jobs: '<path d="M9 6h11M9 12h11M9 18h11"/><path d="m3.5 6 1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2"/>',
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>',
  warning: '<path d="M12 3 2 20h20z"/><path d="M12 9v5M12 17h.01"/>',
  error: '<path d="M8 2h8l6 6v8l-6 6H8l-6-6V8z"/><path d="m15 9-6 6M9 9l6 6"/>',
  done: '<circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/>',
  download: '<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4"/>',
  upload: '<path d="M12 15V3M7 8l5-5 5 5"/><path d="M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4"/>',
  more: '<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/>',
  blank: '',
};

export function icon(name, cls = '') {
  const span = document.createElement('span');
  span.innerHTML = `<svg class="icon ${cls}" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;
  return span.firstElementChild;
}

export function button(label, onClick, { iconName, cls, disabled, title, iconOnly, id } = {}) {
  return h('button', {
    type: 'button', id, class: [cls, iconOnly && 'icon-only'].filter(Boolean).join(' ') || null, disabled,
    title: title ?? (iconOnly ? label : null), 'aria-label': iconOnly ? label : null,
    onclick: (e) => { e.stopPropagation(); onClick?.(e); },
  }, iconName && icon(iconName), !iconOnly && label);
}

/** A frequent secondary action as an icon; while `busy` a spinner stands in for the icon. */
export function iconButton(label, onClick, iconName, { id, disabled, busy, title } = {}) {
  const b = button(label, onClick, { id, iconName: busy ? null : iconName, iconOnly: true, disabled: disabled || busy, title: title ?? label });
  if (busy) b.prepend(h('span', { class: 'spinner', 'aria-hidden': 'true' }));
  return b;
}

// MARK: Menus

let openedMenu = null;
/** The menu button whose menu the last outside click closed, so that click does not open it again. */
let closedByTrigger = null;

export function closeMenu() {
  openedMenu?.remove();
  openedMenu = null;
}

/** Opens a menu below `anchor`; items are [label, action, { iconName, danger, disabled, title }] or '-'. */
export function openMenu(anchor, items) {
  closeMenu();
  const menu = h('div', { class: 'menu', popover: 'manual', role: 'menu' },
    items.filter(Boolean).map((item) => (item === '-' ? h('div', { class: 'menu-separator', role: 'separator' })
      : h('button', { type: 'button', role: 'menuitem', class: item[2]?.danger ? 'danger' : null, disabled: item[2]?.disabled, title: item[2]?.title,
        onclick: () => { closeMenu(); item[1](); } }, icon(item[2]?.iconName ?? 'blank'), item[0]))));
  menu.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      closeMenu();
      anchor.focus();
      return;
    }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const entries = [...menu.querySelectorAll('button:not(:disabled)')];
    const next = entries.indexOf(document.activeElement) + (e.key === 'ArrowDown' ? 1 : -1);
    entries[(next + entries.length) % entries.length]?.focus();
  });
  document.body.append(menu);
  menu.showPopover();
  openedMenu = menu;
  const rect = anchor.getBoundingClientRect();
  const { offsetWidth: width, offsetHeight: height } = menu;
  menu.style.left = `${Math.max(8, Math.min(rect.right - width, innerWidth - width - 8))}px`;
  menu.style.top = `${rect.bottom + 4 + height <= innerHeight ? rect.bottom + 4 : Math.max(8, rect.top - 4 - height)}px`;
  menu.querySelector('button:not(:disabled)')?.focus();
}

// Menus close on clicks outside, when the window loses focus or changes size.
document.addEventListener('pointerdown', (e) => {
  closedByTrigger = null;
  if (!openedMenu || openedMenu.contains(e.target)) return;
  closedByTrigger = e.target.closest?.('[aria-haspopup="menu"]') ?? null;
  closeMenu();
}, true);
window.addEventListener('blur', closeMenu);
window.addEventListener('resize', closeMenu);

/** "⋯": rarer and destructive actions of a view; `items()` is read when it opens. */
export function menuButton(label, items, { id } = {}) {
  const trigger = button(label, () => {
    if (closedByTrigger === trigger) closedByTrigger = null;
    else openMenu(trigger, items());
  }, { id, iconName: 'more', iconOnly: true });
  trigger.setAttribute('aria-haspopup', 'menu');
  return trigger;
}

export const formatTime = (secs) => new Date(secs * 1000).toLocaleString('de-DE', { dateStyle: 'medium', timeStyle: 'short' });

/** `vor 3 Min.` for a Unix time. */
export function ago(secs) {
  const d = Math.max(0, Math.round(Date.now() / 1000 - secs));
  if (d < 45) return 'gerade eben';
  if (d < 3600) return `vor ${Math.round(d / 60)} Min.`;
  if (d < 86400) return `vor ${Math.round(d / 3600)} Std.`;
  return `am ${formatTime(secs)}`;
}

export const short = (sha) => (sha ?? '').slice(0, 7);
/** An id from a key, so focus survives re-rendering. */
export const domId = (prefix, key) => `${prefix}-${String(key).replace(/[^a-z0-9]+/gi, '-')}`;

export async function call(command, args) {
  try {
    return await invoke(command, args);
  } catch (error) {
    showAlert(String(error));
    return null;
  }
}

/** Runs a command; on success returns true, on error shows it and returns false. */
export async function act(command, args) {
  try {
    await invoke(command, args);
    return true;
  } catch (error) {
    showAlert(String(error));
    return false;
  }
}

/** Fetches the view with `get_view` and hands it to `apply`; overlapping requests (events come in
 *  bursts while something runs) are merged into one. */
export function refresher(apply) {
  let refreshing = null;
  let again = false;
  return async function refresh() {
    if (refreshing) {
      again = true;
      return refreshing;
    }
    refreshing = (async () => {
      do {
        again = false;
        apply(await invoke('get_view'));
      } while (again);
    })();
    try {
      await refreshing;
    } finally {
      refreshing = null;
    }
  };
}

export const BANNER_ICONS = { warning: 'warning', error: 'error', update: 'download' };

export function banner(kind, text, actions = []) {
  return h('div', { class: `banner ${kind}`, role: kind === 'error' || kind === 'warning' ? 'alert' : 'status' },
    icon(BANNER_ICONS[kind] ?? 'info'),
    h('div', { class: 'banner-text selectable' }, text),
    actions.map(([label, action, opts]) => button(label, action, opts)));
}

/** The banner for a newer version of `app`; `view.running` (and `view.jobRunning`) hold the install back. */
export function updateBanner(view, app) {
  const u = view.availableUpdate;
  const installing = view.installingUpdate;
  const busy = view.running ? 'Erst nach dem laufenden Schritt' : view.jobRunning ? 'Erst nach dem laufenden Auftrag' : null;
  return h('div', { class: 'banner update', role: 'status' },
    icon('download'),
    h('div', { class: 'banner-text' },
      h('strong', null, `${u.prerelease ? 'Beta: ' : ''}${app} ${u.version} ist verfügbar (installiert: ${view.version})`),
      h('div', { class: 'muted small' }, installing ? 'Paket wird geladen, geprüft und installiert …'
        : `${u.prerelease ? 'Beta-Versionen sind zum Testen gedacht. ' : ''}Die Installation benötigt Administratorrechte, danach startet die App neu.`)),
    button('Versionshinweise', () => call('open_url', { url: u.pageUrl })),
    button('Später', () => call('dismiss_update'), { disabled: installing }),
    button(installing ? 'Wird installiert …' : 'Installieren', () => call('install_update'),
      { cls: 'primary', disabled: installing || !!busy, title: busy }));
}

// MARK: Toasts

let toastText = null;
let toastTimer = null;

/** Notices show briefly at the bottom right, so they are seen wherever the user clicked. */
export function renderToast(view) {
  if (view.notice === toastText) return;
  toastText = view.notice;
  clearTimeout(toastTimer);
  const container = document.getElementById('toasts');
  if (!toastText) return container.replaceChildren();
  const dismiss = () => call('dismiss_notice');
  const later = (ms) => { clearTimeout(toastTimer); toastTimer = setTimeout(dismiss, ms); };
  container.replaceChildren(h('div', { class: 'toast', onmouseenter: () => clearTimeout(toastTimer), onmouseleave: () => later(3000) },
    icon('done', 'green'), h('div', { class: 'toast-text selectable' }, toastText),
    button('Schließen', dismiss, { iconName: 'close', iconOnly: true })));
  later(7000);
}


export const usd = (n) => `${n.toLocaleString('de-DE', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD`;

/** Claude's answer with the little Markdown it writes: paragraphs, headings, "- " lists, **bold**, `code`.
 *  Built as DOM nodes, never as HTML. */
export function markdown(text) {
  const inline = (line) => line.split(/(\*\*[^*]+\*\*|`[^`]+`)/).filter(Boolean).map((part) =>
    part.length > 4 && part.startsWith('**') && part.endsWith('**') ? h('strong', null, part.slice(2, -2))
      : part.length > 2 && part.startsWith('`') && part.endsWith('`') ? h('code', null, part.slice(1, -1))
        : part);
  const blocks = [];
  let list = null;
  for (const line of text.split('\n')) {
    const item = line.match(/^\s*[-*] (.*)$/);
    if (item) {
      if (!list) blocks.push(list = h('ul'));
      list.append(h('li', null, inline(item[1])));
      continue;
    }
    list = null;
    if (!line.trim()) continue;
    const heading = line.match(/^#{1,4} (.*)$/);
    blocks.push(h('p', null, heading ? h('strong', null, inline(heading[1])) : inline(line)));
  }
  return h('div', { class: 'md selectable' }, blocks);
}

export function appendTo(log, lines) {
  const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  log.append(...lines.map((l) => h('span', { class: `l-${l.kind}` }, `${l.text}\n`)));
  if (atBottom) log.scrollTop = log.scrollHeight;
}

// MARK: Dialogs

/** A modal dialog whose content `build` returns; `render` builds it again. A re-render keeps the
 *  scroll position and gives the focus back to the element with the same id (controls that change
 *  the dialog need a stable id). */
export function openDialog(title, build) {
  const dialog = h('dialog', { 'aria-label': title, tabindex: '-1' });
  const close = () => dialog.close();
  // The id that last had the focus; a control disabled while busy gets it back once enabled again.
  let wanted = null;
  const render = () => {
    const focused = dialog.contains(document.activeElement) ? document.activeElement : null;
    if (focused && focused !== dialog) wanted = focused.id || null;
    const focusId = wanted;
    const scroll = dialog.querySelector('.dialog-body')?.scrollTop ?? 0;
    const { body, actions } = build(close, render);
    dialog.replaceChildren(h('form', { method: 'dialog', onsubmit: (e) => e.preventDefault() },
      h('div', { class: 'dialog-body' }, body), h('div', { class: 'dialog-actions' }, actions)));
    const bodyEl = dialog.querySelector('.dialog-body');
    if (bodyEl) bodyEl.scrollTop = scroll;
    if (focused) {
      const again = focusId && dialog.querySelector(`#${CSS.escape(focusId)}`);
      // Without the same element, the dialog itself keeps the focus instead of the page behind it.
      (again && !again.disabled ? again : dialog).focus({ preventScroll: true });
    }
  };
  render();
  dialog.addEventListener('close', () => dialog.remove());
  document.body.append(dialog);
  dialog.showModal();
  return { dialog, close, render };
}

export function showAlert(message) {
  openDialog('Fehler', (close) => ({
    body: [h('h2', null, 'Fehler'), h('div', { class: 'selectable' }, message)],
    actions: [button('OK', close, { cls: 'primary' })],
  }));
}

/** Asks before something that cannot be undone from here; `action` runs on confirmation. Cancel has
 *  the focus, so Enter never confirms by accident. */
export function confirmDialog(title, text, label, action, { danger = false } = {}) {
  const { dialog } = openDialog(title, (close) => ({
    body: [h('h2', null, title), h('p', { class: 'selectable', style: 'margin:0' }, text)],
    actions: [button('Abbrechen', close, { id: 'confirm-cancel' }), button(label, async () => { close(); await action(); }, { cls: danger ? 'primary danger-fill' : 'primary' })],
  }));
  dialog.querySelector('#confirm-cancel')?.focus();
}

export function row(label, control, id) {
  if (id) control.id = id;
  return h('div', { class: 'field-row' }, h('label', { for: control.id || null }, label), control);
}

