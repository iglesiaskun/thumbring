// Small pieces shared by the pages: DOM shortcuts, icons, a slider row, the
// toast and the dialogs.

export const $ = (selector, root = document) => root.querySelector(selector);
export const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

/** Text for HTML. Everything that comes from the settings goes through it. */
export const esc = text => String(text ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

/* ---------- Icons (Lucide, ISC: icons/LICENSE-LUCIDE) ---------- */

const icons = new Map();

/** Reads the icons once, at start, so the pages can draw them right away. */
export async function loadIcons(names) {
  await Promise.all(names.map(async name => {
    const text = await (await fetch(`icons/${name}.svg`)).text();
    const start = text.indexOf('>', text.indexOf('<svg')) + 1;
    icons.set(name, text.slice(start, text.lastIndexOf('</svg>')).trim());
  }));
}

export const icon = (name, cls = '') =>
  `<svg class="ui ${cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons.get(name) ?? ''}</svg>`;

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
  toastTimer = setTimeout(hide, action ? 5200 : 2600);
}

/* ---------- Dialogs ---------- */

let closeDialog = null;

/**
 * Asks something and resolves with the value of the button pressed, or
 * 'cancel' for Esc or a click outside. `body` is HTML.
 */
export const dialog = options => openDialog(options).done;

/**
 * The same, for dialogs that change while open: returns `done` (the
 * promise), `close(value)`, `update({ body, buttons })` and the dialog's
 * element, `root`.
 */
export function openDialog({ title, body = '', buttons }) {
  const scrim = $('#scrim');
  if (closeDialog) closeDialog('cancel');
  let current = buttons;
  scrim.innerHTML = `<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dlg-title">
    <div class="dlg-head"><h2 id="dlg-title">${esc(title)}</h2></div>
    <div class="dlg-body"></div>
    <div class="dlg-foot"></div>
  </div>`;
  const root = $('.dialog', scrim);
  const update = ({ body: html, buttons: list } = {}) => {
    if (html !== undefined) {
      $('.dlg-body', root).innerHTML = html;
      $('.dlg-body', root).hidden = !html;
    }
    if (list) {
      current = list;
      $('.dlg-foot', root).innerHTML = list.map((b, i) =>
        `<button type="button" class="btn ${b.primary ? 'primary' : ''}" data-i="${i}">${esc(b.label)}</button>`).join('');
      ($('.btn.primary', root) ?? $('.btn', root)).focus();
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
    const b = e.target.closest('[data-i]');
    if (b) close(current[Number(b.dataset.i)].value);
    else if (e.target === scrim) close('cancel');
  };
  update({ body, buttons });
  return { root, done, close, update };
}

/** Closes the open dialog as if Esc was pressed. `false` if none was open. */
export function cancelDialog() {
  if (!closeDialog) return false;
  closeDialog('cancel');
  return true;
}
