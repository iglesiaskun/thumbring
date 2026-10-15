// The settings window: navigation, the save bar, and talking to the Rust side
// (src/main.rs) through Tauri's `invoke`.

import { $, $$, cancelDialog, dialog, esc, icon, loadIcons, toast } from './js/ui.js';
import { canRedo, canUndo, change, dirty, onChange, redo, reset, store, undo } from './js/store.js';
import * as behavior from './js/pages/behavior.js';

const PAGES = [behavior];
const ICONS = ['circle-alert', 'undo-2', 'redo-2', ...PAGES.flatMap(p => [p.icon, ...(p.icons ?? [])])];

const invoke = (command, args) => window.__TAURI__.core.invoke(command, args);
const listen = (event, fn) => window.__TAURI__.event.listen(event, fn);

let page = PAGES[0];
let wasDirty = false;

/* ---------- Pages ---------- */

function buildNav() {
  $('#nav-pages').innerHTML = PAGES.map(p =>
    `<button type="button" class="nv" data-page="${p.id}">${icon(p.icon)}${esc(p.title)}</button>`).join('');
  $('#nav-pages').addEventListener('click', e => {
    const b = e.target.closest('[data-page]');
    if (b) show(PAGES.find(p => p.id === b.dataset.page));
  });
  $('#content').innerHTML = PAGES.map(p => `<section class="page" id="pg-${p.id}" hidden></section>`).join('');
  for (const p of PAGES) p.mount($(`#pg-${p.id}`));
}

function show(next) {
  page = next;
  for (const b of $$('.nv')) {
    if (b.dataset.page === page.id) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  for (const p of PAGES) $(`#pg-${p.id}`).hidden = p !== page;
  page.render($(`#pg-${page.id}`));
  $('#content').scrollTop = 0;
}

/* ---------- Save bar and banner ---------- */

function updateSaveBar() {
  const d = dirty(), status = $('#status');
  status.classList.toggle('clean', !d);
  const name = store.path.split(/[\\/]/).pop();
  $('.txt', status).innerHTML = d
    ? 'Unsaved changes'
    : store.newFile
      ? `Thumbring uses its defaults; saving creates <code title="${esc(store.path)}">${esc(name)}</code>`
      : `Saved in <code title="${esc(store.path)}">${esc(name)}</code>`;
  $('#btn-save').disabled = !d && !store.newFile;
  $('#btn-discard').disabled = !d;
  $('#btn-undo').disabled = !canUndo();
  $('#btn-redo').disabled = !canRedo();
  if (d !== wasDirty) {
    wasDirty = d;
    invoke('set_dirty', { dirty: d });
  }
}

function showWarnings() {
  const n = store.warnings.length, banner = $('#banner');
  banner.hidden = n === 0;
  if (!n) return;
  banner.innerHTML = `${icon('circle-alert')}
    <span class="bt">Thumbring found ${n === 1 ? 'a problem' : `${n} problems`} in the settings file and uses the default instead. Saving leaves ${n === 1 ? 'that line' : 'those lines'} as ${n === 1 ? 'it is' : 'they are'}.</span>
    <button type="button" class="btn sm" id="ban-show">Show</button>`;
  $('#ban-show').onclick = () => dialog({
    title: 'Problems in the settings file',
    body: `<ul>${store.warnings.map(w => `<li><code>${esc(w)}</code></li>`).join('')}</ul>`,
    buttons: [{ label: 'Close', primary: true, value: 'close' }],
  });
}

function showChangedOutside() {
  const banner = $('#banner');
  banner.hidden = false;
  banner.innerHTML = `${icon('circle-alert')}
    <span class="bt">The settings file changed outside this window after it was opened.</span>
    <button type="button" class="btn sm" id="ban-load">Load the file</button>
    <button type="button" class="btn sm" id="ban-keep">Save mine anyway</button>`;
  $('#ban-load').onclick = () => open();
  $('#ban-keep').onclick = () => save(true);
}

/* ---------- Reading and saving ---------- */

async function open() {
  try {
    reset(await invoke('open_settings'));
    showWarnings();
    return true;
  } catch (e) {
    $('#banner').hidden = true;
    $('#content').innerHTML = `<div class="card problem">
      <h1>The settings file can’t be read</h1>
      <p>${esc(e.reason)}</p>
      <p class="muted">Fix it in a text editor, or delete it to start over from the defaults: <code>${esc(e.path)}</code></p>
      <div><button type="button" class="btn primary" id="retry">Try again</button></div>
    </div>`;
    $('#retry').onclick = () => location.reload();
    return false;
  }
}

async function save(overwrite = false) {
  try {
    reset(await invoke('save_settings', { settings: store.settings, overwrite }));
    showWarnings();
    toast('Saved. Thumbring is using the new settings.');
    return true;
  } catch (e) {
    if (e.kind === 'changed_outside') showChangedOutside();
    else if (e.kind === 'invalid') {
      await dialog({
        title: 'These settings can’t be saved',
        body: `<p>Thumbring would not accept them:</p><ul>${e.details.map(d => `<li><code>${esc(d)}</code></li>`).join('')}</ul>`,
        buttons: [{ label: 'OK', primary: true, value: 'ok' }],
      });
    } else toast(`Not saved: ${e.details}`);
    return false;
  }
}

async function discard() {
  const discarded = JSON.parse(JSON.stringify(store.settings));
  if (await open()) {
    // Undo brings the discarded changes back, as one change.
    toast('Changes discarded', { action: 'Undo', run: () => change(s => Object.assign(s, discarded)) });
  }
}

async function askBeforeClosing() {
  if (!dirty()) return invoke('close_window');
  const choice = await dialog({
    title: 'Save your changes?',
    body: '<p>If you close without saving, Thumbring keeps the settings it has now.</p>',
    buttons: [
      { label: 'Cancel', value: 'cancel' },
      { label: 'Don’t save', value: 'discard' },
      { label: 'Save', primary: true, value: 'save' },
    ],
  });
  if (choice === 'discard' || (choice === 'save' && await save())) invoke('close_window');
}

/* ---------- Keyboard ---------- */

const typing = () => document.activeElement?.matches('input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea, select');

window.addEventListener('keydown', e => {
  const mod = e.ctrlKey || e.metaKey, key = e.key.toLowerCase();
  if (mod && key === 's') { e.preventDefault(); if (!$('#btn-save').disabled) save(); return; }
  if (e.key === 'Escape') { cancelDialog(); return; }
  // Browser shortcuts that make no sense in a settings window.
  if (e.key === 'F5' || (mod && ['r', 'p', 'f', 'g', 'j', 'u'].includes(key))) { e.preventDefault(); return; }
  if (!$('#scrim').hidden || typing()) return;
  if (mod && !e.shiftKey && key === 'z') { e.preventDefault(); if (undo()) toast('Undone'); }
  else if (mod && (key === 'y' || (e.shiftKey && key === 'z'))) { e.preventDefault(); if (redo()) toast('Redone'); }
});
window.addEventListener('contextmenu', e => { if (!typing()) e.preventDefault(); });
// The mouse's Back and Forward buttons must never navigate the page.
for (const type of ['mousedown', 'mouseup', 'auxclick']) {
  window.addEventListener(type, e => { if (e.button === 3 || e.button === 4) e.preventDefault(); });
}

/* ---------- Start ---------- */

async function start() {
  await loadIcons(ICONS);
  $('#btn-undo').innerHTML = icon('undo-2');
  $('#btn-redo').innerHTML = icon('redo-2');
  $('#btn-save').onclick = () => save();
  $('#btn-discard').onclick = discard;
  $('#btn-undo').onclick = () => undo() && toast('Undone');
  $('#btn-redo').onclick = () => redo() && toast('Redone');
  onChange(render => {
    if (render) page.render($(`#pg-${page.id}`));
    updateSaveBar();
  });
  listen('close-requested', askBeforeClosing);
  buildNav();
  if (await open()) show(page);
}

// The window starts hidden; it appears even if something above failed, so
// a problem never leaves an invisible window behind.
start()
  .catch(e => {
    console.error(e);
    $('#content').innerHTML = `<div class="card problem"><h1>Something went wrong</h1><p><code>${esc(e?.message ?? e)}</code></p></div>`;
  })
  .finally(() => invoke('ready'));
