// Small pieces shared by the pages: DOM shortcuts, icons, a slider row, the
// toast, the dialogs and the popovers.

export const $ = (selector, root = document) => root.querySelector(selector);
export const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

/** Text for HTML. Everything that comes from the settings goes through it. */
export const esc = text => String(text ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

/** Shows another page: 'ring', 'gestures', 'apps' or 'behavior'. */
export const goto = page => window.dispatchEvent(new CustomEvent('thumbring:goto', { detail: page }));

/** Focus is in a text field, where keys type instead of acting. */
export const isTyping = () =>
  !!document.activeElement?.matches('input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea, select');

/* ---------- Icons (Lucide, ISC: icons/LICENSE-LUCIDE) ---------- */

/** The drawing inside an SVG file: what goes between its <svg> tags. */
export function svgInner(text) {
  const start = text.indexOf('>', text.indexOf('<svg')) + 1;
  return text.slice(start, text.lastIndexOf('</svg>')).trim();
}

export const svg = (inner, cls = '') =>
  `<svg class="${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;

const icons = new Map();

/** Reads the icons once, at start, so the pages can draw them right away. */
export async function loadIcons(names) {
  await Promise.all([...new Set(names)].map(async name => {
    icons.set(name, svgInner(await (await fetch(`icons/${name}.svg`)).text()));
  }));
}

export const icon = (name, cls = '') => svg(icons.get(name) ?? '', `ui ${cls}`);

/* ---------- Controls ---------- */

/**
 * A labelled slider. `path` is the key path in the settings; the value,
 * range and default come from the caller.
 */
export function sliderRow({ path, label, value, min, max, step, format, note, defaultValue }) {
  const id = 'sl-' + path.replace(/\W/g, '-');
  const changed = defaultValue !== undefined && value !== defaultValue;
  return `<div class="srow">
    <label for="${id}">${esc(label)}</label>
    <output id="${id}-out" for="${id}" class="${changed ? 'changed' : ''}" title="Default: ${esc(format(defaultValue))}">${esc(format(value))}</output>
    <input type="range" id="${id}" data-path="${esc(path)}" min="${min}" max="${max}" step="${step}" value="${value}">
    ${note ? `<small>${esc(note)}</small>` : ''}
  </div>`;
}

/* ---------- Toast ---------- */

let toastTimer = 0;

/** A short message above the save bar, with an optional action such as Undo. */
export function toast(text, { action, run } = {}) {
  const el = $('#toast'), button = $('button', el);
  const hide = () => el.classList.remove('show');
  $('.t', el).textContent = text;
  button.hidden = !action;
  button.textContent = action ?? '';
  button.onclick = action ? () => { hide(); run(); } : null;
  el.classList.add('show');
  clearTimeout(toastTimer);
  // Long enough to read: longer texts stay longer.
  toastTimer = setTimeout(hide, action ? 5200 : Math.max(2600, text.length * 50));
}

/* ---------- Dialogs ---------- */

let closeDialog = null;

/**
 * Asks something and resolves with the value of the button pressed, or
 * 'cancel' for Esc or a click outside. `body` is HTML.
 */
export const dialog = options => openDialog(options).done;

/**
 * The same, for dialogs that change while open. Returns `done` (the
 * promise), `close(value)`, `update({ body, buttons, note })` and the
 * dialog's element, `root`. `note` is HTML at the start of the button row;
 * a button with `disabled: true` cannot be pressed; `wide` makes room for
 * more.
 */
export function openDialog({ title, body = '', buttons, note = '', wide = false }) {
  closePop();
  const scrim = $('#scrim');
  if (closeDialog) closeDialog('cancel');
  let current = buttons;
  scrim.innerHTML = `<div class="dialog ${wide ? 'wide' : ''}" role="dialog" aria-modal="true" aria-labelledby="dlg-title">
    <div class="dlg-head"><h2 id="dlg-title">${esc(title)}</h2></div>
    <div class="dlg-body"></div>
    <div class="dlg-foot"><span class="dlg-note"></span><span class="dlg-buttons"></span></div>
  </div>`;
  const root = $('.dialog', scrim);
  const update = ({ body: html, buttons: list, note: noteHtml } = {}) => {
    if (html !== undefined) {
      $('.dlg-body', root).innerHTML = html;
      $('.dlg-body', root).hidden = !html;
    }
    if (noteHtml !== undefined) $('.dlg-note', root).innerHTML = noteHtml;
    if (list) {
      const hadFocus = root.contains(document.activeElement) && document.activeElement.closest('.dlg-buttons');
      current = list;
      $('.dlg-buttons', root).innerHTML = list.map((b, i) =>
        `<button type="button" class="btn ${b.primary ? 'primary' : ''}" data-i="${i}" ${b.disabled ? 'disabled' : ''}>${esc(b.label)}</button>`).join('');
      if (hadFocus || !root.contains(document.activeElement)) {
        ($('.btn.primary:not(:disabled)', root) ?? $('.dlg-buttons .btn', root))?.focus();
      }
    }
  };
  scrim.hidden = false;
  const focused = document.activeElement;
  let close;
  const done = new Promise(resolve => {
    close = closeDialog = value => {
      if (closeDialog !== close) return;
      closeDialog = null;
      scrim.hidden = true;
      scrim.innerHTML = '';
      if (focused && document.contains(focused)) focused.focus();
      resolve(value);
    };
  });
  scrim.onclick = e => {
    const b = e.target.closest('.dlg-buttons [data-i]');
    if (b) close(current[Number(b.dataset.i)].value);
    else if (e.target === scrim) close('cancel');
  };
  update({ body, buttons, note });
  return { root, done, close, update };
}

/** Closes the open dialog as if Esc was pressed. `false` if none was open. */
export function cancelDialog() {
  if (!closeDialog) return false;
  closeDialog('cancel');
  return true;
}

/* ---------- Popovers ---------- */

let popAnchor = null;

/**
 * A small panel under `anchor` (above it if there is no room). `bind(root)`
 * adds its listeners; it closes with Esc, a click outside or `closePop()`.
 */
export function openPop(anchor, html, bind) {
  closePop();
  const pop = $('#pop');
  const inner = document.createElement('div');
  inner.innerHTML = html;
  pop.appendChild(inner);
  pop.hidden = false;
  const r = anchor.getBoundingClientRect(), width = pop.offsetWidth, height = pop.offsetHeight;
  const x = Math.min(Math.max(8, r.left), Math.max(8, innerWidth - width - 8));
  let y = r.bottom + 6;
  if (y + height > innerHeight - 8) y = Math.max(8, r.top - height - 6);
  pop.style.left = x + 'px';
  pop.style.top = y + 'px';
  popAnchor = anchor;
  bind(inner);
  ($('[aria-pressed="true"]', inner) ?? $('input, button', inner))?.focus({ preventScroll: true });
}

/** Closes the open popover. `false` if none was open. */
export function closePop() {
  const pop = $('#pop');
  if (!pop || pop.hidden) return false;
  pop.hidden = true;
  pop.textContent = '';
  if (popAnchor && document.contains(popAnchor)) popAnchor.focus({ preventScroll: true });
  popAnchor = null;
  return true;
}

document.addEventListener('pointerdown', e => {
  const pop = $('#pop');
  if (pop && !pop.hidden && !pop.contains(e.target) && !popAnchor?.contains(e.target)) closePop();
}, true);
window.addEventListener('resize', () => closePop());
