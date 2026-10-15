// Ring: the 8 slots and their folders, edited on a live preview.
//
// Drag an action from the list onto a slot or into a folder, drag items to
// move or swap them, click an item to change its name, icon and command,
// and remove it with the × on the item under the cursor or with Del.

import { $, $$, esc, icon as drawIcon, isTyping, openPop, closePop, toast } from '../ui.js';
import { change, store, undo } from '../store.js';
import { createRingStage, STAGE_ICONS } from '../ring-stage.js';
import { itemGlyph, ringIconNames, ringSvg } from '../ring-icons.js';
import { BUTTON_NAMES, COMMAND_ICONS, commandHTML, commandShort } from '../commands.js';
import { BUILDER_ICONS, chooseCommand } from '../command-builder.js';
import { browse } from '../system.js';

export const id = 'ring';
export const title = 'Ring';
export const icon = 'circle-dot';
export const icons = [...STAGE_ICONS, ...COMMAND_ICONS, ...BUILDER_ICONS, 'chevron-left', 'chevron-right', 'trash-2', 'folder-plus', 'arrow-up', 'arrow-down', 'rotate-ccw'];

const SLOT_POS = ['top', 'top right', 'right', 'bottom right', 'bottom', 'bottom left', 'left', 'top left'];
const FOLDER_MAX = 9;
const action = (name, ringIcon, command) => ({ name, icon: ringIcon, command });

/** Ready-made actions to drag onto the ring. */
const CATALOG = [
  ['Media', [action('Play or pause', 'play', 'PlayPause'), action('Next track', 'skip-forward', 'NextTrack'), action('Previous track', 'skip-back', 'PrevTrack'), action('Volume up', 'volume-2', 'VolumeUp'), action('Volume down', 'volume-1', 'VolumeDown'), action('Mute', 'volume-x', 'VolumeMute')]],
  ['Editing', [action('Copy', 'copy', 'Ctrl+C'), action('Paste', 'clipboard-paste', 'Ctrl+V'), action('Cut', 'scissors', 'Ctrl+X'), action('Undo', 'undo-2', 'Ctrl+Z'), action('Redo', 'redo-2', 'Ctrl+Y'), action('Clipboard history', 'clipboard-list', 'Win+V')]],
  ['Windows', [action('Screenshot', 'scan', 'screenshot'), action('Emoji', 'smile', 'emoji'), action('File Explorer', 'folder-open', 'explorer'), action('Lock screen', 'lock', 'lock'), action('Task View', 'layout-grid', 'Win+Tab'), action('Settings', 'settings', 'open:ms-settings:')]],
  ['Apps', [action('Calculator', 'calculator', 'open:calc.exe'), action('Notepad', 'sticky-note', 'open:notepad.exe'), action('Terminal', 'terminal', 'open:wt.exe'), action('Mail', 'mail', 'open:mailto:')]],
  ['Your own', [{ name: 'New action', own: true }, { name: 'New folder', icon: 'folder-open', folder: true, own: true }]],
];

let root = null, stage = null, sel = null, drag = null;
const slots = () => store.settings.ring.slots;

/* ---------- Selection ---------- */

function selItem() {
  if (!sel) return null;
  const s = slots()[sel.slot];
  if (!s) return null;
  if (sel.index == null) return s;
  return s.type === 'folder' ? s.items[sel.index] ?? null : null;
}

function fixSelection() {
  if (sel && !selItem()) sel = sel.index != null && slots()[sel.slot]?.type === 'folder' ? { slot: sel.slot } : null;
}

function select(next, focusName = false) {
  sel = next;
  fixSelection();
  stage.setSelection(sel);
  renderInspector();
  if (focusName) { const n = $('#insp-name', root); if (n) { n.focus(); n.select(); } }
}

/** The selected item inside settings `s`, which may be a copy. */
const selTarget = s => (sel.index != null ? s.ring.slots[sel.slot].items[sel.index] : s.ring.slots[sel.slot]);
const pickAction = s => {
  const a = { name: s.name };
  if (s.icon) a.icon = s.icon;
  if (s.command != null) a.command = s.command;
  return a;
};

/** Items without a name: the engine would leave them out. */
export function problems() {
  const out = [];
  slots().forEach((s, i) => {
    if (s.type === 'empty') return;
    if (!s.name.trim()) out.push({ message: `The ${s.type} in the ${SLOT_POS[i]} slot needs a name.`, at: { slot: i } });
    if (s.type === 'folder') {
      s.items.forEach((it, j) => {
        if (!it.name.trim()) out.push({ message: `Action ${j + 1} in ${s.name || 'a folder'} needs a name.`, at: { slot: i, index: j } });
      });
    }
  });
  return out;
}

/** Selects the item a problem is about. */
export function reveal(problem) {
  sel = problem.at;
}

/* ---------- Page ---------- */

export function mount(pageRoot) {
  root = pageRoot;
  root.innerHTML = `
    <header class="ph"><h1>${title}</h1><p>Hold <span id="ring-button">Forward</span> to open it. Drag actions onto the slots, or click an item to change it.</p></header>
    <div class="ring-grid">
      <div class="ring-main">
        <div class="stage" id="ringStage" aria-label="Ring preview. Click an item to change it.">
          <canvas></canvas>
          <div class="layer overlay"></div>
          <div class="layer icons"></div>
          <div class="label"><span class="nm"></span><small class="k"></small></div>
          <p class="hint">Click an item to change it · drag it to move it · × removes it · dashed circles are empty slots</p>
        </div>
        <section class="catalog" aria-labelledby="h-cat">
          <div class="cat-head"><h2 id="h-cat">Actions</h2><p>Drag one onto a slot or into a folder. A click adds it to the selected slot or folder.</p></div>
          <div class="cat-groups">${CATALOG.map(([group, items], gi) => `<div class="cat-group"><h3>${group}</h3><div class="chips">${items.map((it, i) =>
            `<button type="button" class="chip ${it.own ? 'own' : ''}" data-g="${gi}" data-i="${i}" title="${esc(it.folder ? 'A folder for up to 9 actions' : it.own ? 'An action with nothing in it yet' : commandShort(it.command))}"><span class="mini">${it.own && !it.folder ? drawIcon('plus') : itemGlyph(it)}</span>${esc(it.name)}</button>`).join('')}</div></div>`).join('')}</div>
        </section>
      </div>
      <aside class="inspector" id="inspector" aria-label="Selected item"></aside>
    </div>`;

  stage = createRingStage($('#ringStage', root), {
    mode: 'edit',
    getSlots: () => slots(),
    getAnim: () => store.settings.animation,
    reduced: () => matchMedia('(prefers-reduced-motion: reduce)').matches && !store.settings.animation.ignore_system_reduced_motion,
    isDragging: () => !!drag?.started,
    onPointerDown: (t, e) => {
      if (t.kind === 'action' || t.kind === 'folder') dragPrepare(e, { kind: 'slot', slot: t.slot }, () => select({ slot: t.slot }));
      else if (t.kind === 'child') dragPrepare(e, { kind: 'child', slot: t.slot, index: t.index }, () => select({ slot: t.slot, index: t.index }));
      else if (t.kind === 'empty') select({ slot: t.slot });
      else if (t.kind === 'ghost') addToFolder(t.slot);
      else select(null);
    },
    onRemove: t => removeAt(t),
  });

  const catalog = $('.catalog', root), inspector = $('#inspector', root);
  catalog.addEventListener('pointerdown', e => {
    const b = e.target.closest('.chip');
    if (!b || e.button !== 0) return;
    const p = catalogPayload(b);
    dragPrepare(e, p, () => quickAdd(p));
  });
  catalog.addEventListener('keydown', e => {
    const b = e.target.closest('.chip');
    if (!b || (e.key !== 'Enter' && e.key !== ' ')) return;
    e.preventDefault();
    quickAdd(catalogPayload(b));
  });
  inspector.addEventListener('input', onInspectorInput);
  inspector.addEventListener('change', onInspectorChange);
  inspector.addEventListener('click', onInspectorClick);
  window.addEventListener('keydown', e => {
    // A key the window already used (Esc closing a dialog) does nothing here.
    if (e.defaultPrevented || root.hidden || !$('#scrim').hidden || isTyping()) return;
    if (sel && (e.key === 'Delete' || e.key === 'Backspace')) { e.preventDefault(); removeSelected(); }
    else if (e.key === 'Escape' && drag) dragCleanup();
    else if (e.key === 'Escape' && sel) select(null);
  });
}

export function render() {
  $('#ring-button', root).textContent = BUTTON_NAMES[store.settings.ring.button];
  fixSelection();
  stage.rebuild();
  stage.setSelection(sel);
  renderInspector();
}

/** The page is on screen: the ring opens with its animation. */
export function shown() {
  stage.show();
}

/* ---------- Inspector ---------- */

function typeSeg(kind) {
  return `<div class="seg typeseg" role="radiogroup" aria-label="Kind of slot">
    <input type="radio" name="insp-type" id="it-action" value="action" ${kind === 'action' ? 'checked' : ''}><label for="it-action">Action</label>
    <input type="radio" name="insp-type" id="it-folder" value="folder" ${kind === 'folder' ? 'checked' : ''}><label for="it-folder">Folder</label>
  </div>`;
}

function nameField(it, hint) {
  const bad = !it.name.trim();
  return `<div class="field"><label class="flabel" for="insp-name">Name</label>
    <input class="input" id="insp-name" value="${esc(it.name)}" maxlength="40" autocomplete="off" spellcheck="false" ${bad ? 'aria-invalid="true"' : ''}>
    <small class="fhint ${bad ? 'bad' : ''}" id="insp-name-hint" data-hint="${esc(hint)}">${bad ? 'Give it a name: it is the label next to the item.' : esc(hint)}</small></div>`;
}

function iconField(it) {
  return `<div class="field"><span class="flabel" id="lbl-icon">Icon</span>
    <button type="button" class="icon-btn" data-act="icon" aria-labelledby="lbl-icon"><span class="icon-prev">${itemGlyph(it)}</span><span class="nm">${it.icon ? esc(it.icon) : 'First letter of the name'}</span><span class="go">Change</span></button></div>`;
}

function commandField(it) {
  return `<div class="field"><span class="flabel" id="lbl-cmd">Command</span>
    <button type="button" class="cmd-btn" data-act="cmd" aria-labelledby="lbl-cmd">${commandHTML(it.command ?? null)}${drawIcon('chevron-right', 'go')}</button></div>`;
}

function renderInspector() {
  const el = $('#inspector', root), it = selItem();
  if (!sel || !it) {
    const S = slots(), folders = S.filter(s => s.type === 'folder');
    const actions = S.filter(s => s.type === 'action').length + folders.reduce((n, f) => n + f.items.length, 0);
    const empty = S.filter(s => s.type === 'empty').length;
    el.innerHTML = `<div class="insp-top"><p class="insp-eyebrow">Nothing selected</p><h2 class="insp-title">Your ring</h2></div>
      <p class="muted small">8 slots, starting at the top, clockwise: ${actions} ${actions === 1 ? 'action' : 'actions'}, ${folders.length} ${folders.length === 1 ? 'folder' : 'folders'}${empty ? `, ${empty} empty` : ''}.</p>
      <ul class="tips"><li><b>Click</b> an item to change its name, icon and command.</li><li><b>Drag</b> an action from the list onto a slot, or into a folder.</li><li><b>Drag</b> an item onto another slot to swap them.</li><li>The <b>×</b> on the item under the cursor removes it, and so does <kbd>Del</kbd> on the selected one. <kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes.</li></ul>
      <div class="insp-foot"><button type="button" class="btn" data-act="restore">${drawIcon('rotate-ccw')}Restore the default ring</button></div>`;
    return;
  }
  const s = slots()[sel.slot];
  if (sel.index != null) {
    const n = s.items.length;
    el.innerHTML = `<div class="insp-top"><button type="button" class="backlink" data-act="back">${drawIcon('chevron-left')}${esc(s.name || 'Folder')}</button>
        <p class="insp-eyebrow">Action ${sel.index + 1} of ${n} in this folder</p></div>
      ${nameField(it, 'Shown next to the action when you point at it.')}
      ${iconField(it)}
      ${commandField(it)}
      <div class="insp-foot">
        <button type="button" class="btn icon" data-act="move-prev" aria-label="Move back along the arc" title="Move back along the arc" ${sel.index === 0 ? 'disabled' : ''}>${drawIcon('chevron-left')}</button>
        <button type="button" class="btn icon" data-act="move-next" aria-label="Move forward along the arc" title="Move forward along the arc" ${sel.index === n - 1 ? 'disabled' : ''}>${drawIcon('chevron-right')}</button>
        <span class="spacer"></span>
        <button type="button" class="btn danger" data-act="remove">${drawIcon('trash-2')}Remove</button>
      </div>`;
    return;
  }
  if (s.type === 'empty') {
    el.innerHTML = `<div class="insp-top"><p class="insp-eyebrow">Slot · ${SLOT_POS[sel.slot]}</p><h2 class="insp-title">Empty</h2></div>
      <p class="muted small">Nothing shows up here in the ring. Drag an action from the list onto it, or start one here.</p>
      <div class="btnrow"><button type="button" class="btn" data-act="new-action">${drawIcon('plus')}New action</button><button type="button" class="btn" data-act="new-folder">${drawIcon('folder-plus')}New folder</button></div>`;
    return;
  }
  if (s.type === 'folder') {
    const n = s.items.length;
    el.innerHTML = `<div class="insp-top"><p class="insp-eyebrow">Folder · ${SLOT_POS[sel.slot]} slot</p></div>
      ${typeSeg('folder')}
      ${nameField(s, 'For you only: folders show no label in the ring.')}
      ${iconField(s)}
      <div class="field"><div class="flabel-row"><span class="flabel">Actions</span><span class="count">${n} of ${FOLDER_MAX}</span></div>
        ${n ? `<ol class="flist">${s.items.map((a, i) => `<li>
          <button type="button" class="frow" data-act="f-open" data-i="${i}"><span class="mini">${itemGlyph(a)}</span><span class="nm">${esc(a.name || 'Untitled')}</span><span class="sub">${esc(commandShort(a.command ?? null))}</span></button>
          <span class="ord"><button type="button" class="btn subtle icon" data-act="f-up" data-i="${i}" aria-label="Move ${esc(a.name)} up" ${i === 0 ? 'disabled' : ''}>${drawIcon('arrow-up')}</button><button type="button" class="btn subtle icon" data-act="f-down" data-i="${i}" aria-label="Move ${esc(a.name)} down" ${i === n - 1 ? 'disabled' : ''}>${drawIcon('arrow-down')}</button></span>
        </li>`).join('')}</ol>` : '<p class="fhint">An empty folder opens to nothing. Add an action, or drag one onto it.</p>'}
        <button type="button" class="btn start" data-act="add-action" ${n >= FOLDER_MAX ? 'disabled' : ''}>${drawIcon('plus')}Add action</button>
        ${n >= FOLDER_MAX ? `<small class="fhint">A folder holds up to ${FOLDER_MAX} actions.</small>` : ''}
      </div>
      <div class="insp-foot"><span class="spacer"></span><button type="button" class="btn danger" data-act="remove">${drawIcon('trash-2')}Remove folder</button></div>`;
    return;
  }
  el.innerHTML = `<div class="insp-top"><p class="insp-eyebrow">Action · ${SLOT_POS[sel.slot]} slot</p></div>
    ${typeSeg('action')}
    ${nameField(s, 'Shown next to the action when you point at it.')}
    ${iconField(s)}
    ${commandField(s)}
    <div class="insp-foot"><span class="spacer"></span><button type="button" class="btn danger" data-act="remove">${drawIcon('trash-2')}Remove</button></div>`;
}

function onInspectorInput(e) {
  if (e.target.id !== 'insp-name' || !sel) return;
  const value = e.target.value, at = { ...sel };
  // Typing keeps the field (and its focus): only the preview is drawn again.
  change(s => { selTarget(s).name = value; }, { key: `name ${at.slot} ${at.index}`, render: false });
  stage.rebuild();
  const bad = !value.trim(), hint = $('#insp-name-hint', root);
  e.target.toggleAttribute('aria-invalid', bad);
  hint.classList.toggle('bad', bad);
  hint.textContent = bad ? 'Give it a name: it is the label next to the item.' : hint.dataset.hint;
  const it = selItem(), prev = $('.icon-prev', root);
  if (prev && it && !it.icon) prev.innerHTML = itemGlyph(it);
}

function onInspectorChange(e) {
  if (e.target.name !== 'insp-type' || !sel) return;
  const to = e.target.value, slot = sel.slot, s = slots()[slot];
  if (to === 'folder' && s.type === 'action') {
    change(c => { c.ring.slots[slot] = { type: 'folder', name: s.name, ...(s.icon ? { icon: s.icon } : {}), items: [pickAction(s)] }; });
    toast(`“${s.name}” is now a folder, with the action inside`, { action: 'Undo', run: undoLast });
  } else if (to === 'action' && s.type === 'folder') {
    const lost = s.items.length;
    change(c => { c.ring.slots[slot] = { type: 'action', name: s.name, ...(s.icon ? { icon: s.icon } : {}) }; });
    if (lost) toast(`The folder became an action; its ${lost} ${lost === 1 ? 'action was' : 'actions were'} removed`, { action: 'Undo', run: undoLast });
  }
  select({ slot });
}

async function onInspectorClick(e) {
  const b = e.target.closest('[data-act]');
  if (!b) return;
  const a = b.dataset.act, i = Number(b.dataset.i);
  if (a === 'restore') {
    change(s => { s.ring.slots = structuredClone(store.defaults.ring.slots); });
    select(null);
    toast('The default ring is back', { action: 'Undo', run: undoLast });
  } else if (a === 'back') select({ slot: sel.slot });
  else if (a === 'icon') {
    const at = { ...sel };
    pickIcon(b, selItem().icon, name => change(s => {
      const target = at.index != null ? s.ring.slots[at.slot].items[at.index] : s.ring.slots[at.slot];
      if (name) target.icon = name; else delete target.icon;
    }));
  } else if (a === 'cmd') {
    const it = selItem(), at = { ...sel };
    const value = await chooseCommand({ title: `What does “${it.name || 'this action'}” do?`, value: it.command ?? null, mode: 'ring', browse });
    if (value === undefined) return;
    change(s => {
      const target = at.index != null ? s.ring.slots[at.slot].items[at.index] : s.ring.slots[at.slot];
      target.command = value;
    });
  } else if (a === 'remove') removeSelected();
  else if (a === 'move-prev' || a === 'move-next') {
    const slot = sel.slot, from = sel.index, to = from + (a === 'move-prev' ? -1 : 1);
    change(s => { const items = s.ring.slots[slot].items; items.splice(to, 0, items.splice(from, 1)[0]); });
    select({ slot, index: to });
  } else if (a === 'new-action') {
    const slot = sel.slot;
    change(s => { s.ring.slots[slot] = { type: 'action', name: 'New action' }; });
    select({ slot }, true);
  } else if (a === 'new-folder') {
    const slot = sel.slot;
    change(s => { s.ring.slots[slot] = { type: 'folder', name: 'New folder', icon: 'folder-open', items: [] }; });
    select({ slot }, true);
  } else if (a === 'add-action') addToFolder(sel.slot);
  else if (a === 'f-open') select({ slot: sel.slot, index: i });
  else if (a === 'f-up' || a === 'f-down') {
    const slot = sel.slot, to = i + (a === 'f-up' ? -1 : 1);
    change(s => { const items = s.ring.slots[slot].items; items.splice(to, 0, items.splice(i, 1)[0]); });
    const next = $(`[data-act="${a}"][data-i="${to}"]`, root);
    if (next && !next.disabled) next.focus();
  }
}

function pickIcon(anchor, current, onPick) {
  const names = ringIconNames();
  const html = `<p class="pop-title">Icon</p><div class="icongrid">${names.map(n =>
      `<button type="button" data-icon="${n}" title="${n}" aria-label="${n}" aria-pressed="${current === n}">${ringSvg(n)}</button>`).join('')}
    <button type="button" data-icon="" class="aa" title="First letter of the name" aria-label="First letter of the name" aria-pressed="${!current}">Aa</button></div>
    <p class="pop-note">The icons the ring can draw. Aa shows the first letter of the name.</p>`;
  openPop(anchor, html, el => {
    el.addEventListener('click', e => {
      const b = e.target.closest('[data-icon]');
      if (!b) return;
      closePop();
      onPick(b.dataset.icon || null);
    });
    el.addEventListener('keydown', e => {
      const buttons = $$('[data-icon]', el), i = buttons.indexOf(document.activeElement);
      const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 6, ArrowUp: -6 }[e.key];
      if (i >= 0 && step) { e.preventDefault(); buttons[Math.max(0, Math.min(buttons.length - 1, i + step))].focus(); }
    });
  });
}

/* ---------- Adding and removing ---------- */

const undoLast = () => undo();

function addToFolder(slot) {
  const f = slots()[slot];
  if (f?.type !== 'folder') return;
  if (f.items.length >= FOLDER_MAX) { toast(`${f.name} is full: a folder holds up to ${FOLDER_MAX} actions`); return; }
  const at = f.items.length;
  change(s => { s.ring.slots[slot].items.push({ name: 'New action' }); });
  select({ slot, index: at }, true);
}

/** Removes an action, a folder (with its actions) or an action inside a folder. */
function removeAt(t) {
  const slot = t.slot, child = t.kind === 'child', index = child ? t.index : null, s = slots()[slot];
  if (!s || s.type === 'empty') return;
  const it = child ? s.items[index] : s;
  if (!it) return;
  const name = it.name || 'Untitled', n = s.type === 'folder' ? s.items.length : 0;
  const message = child ? `Removed “${name}” from ${s.name || 'the folder'}`
    : s.type === 'folder' ? `Removed the folder “${name}”${n ? ` and its ${n} ${n === 1 ? 'action' : 'actions'}` : ''}`
    : `Removed “${name}”`;
  // The selection stays where it was, adjusted for the item that left.
  let next = sel;
  if (sel?.slot === slot) {
    if (!child || sel.index === index) next = { slot };
    else if (sel.index != null && sel.index > index) next = { slot, index: sel.index - 1 };
  }
  change(c => { if (child) c.ring.slots[slot].items.splice(index, 1); else c.ring.slots[slot] = { type: 'empty' }; });
  select(next);
  toast(message, { action: 'Undo', run: undoLast });
}

function removeSelected() {
  if (sel) removeAt(sel.index != null ? { kind: 'child', slot: sel.slot, index: sel.index } : { kind: 'slot', slot: sel.slot });
}

function catalogPayload(button) {
  const it = CATALOG[button.dataset.g][1][button.dataset.i];
  if (it.folder) return { kind: 'catalog', folder: true, item: { name: 'New folder', icon: 'folder-open' } };
  return { kind: 'catalog', item: it.own ? { name: 'New action' } : pickAction(it) };
}

const newFolder = () => ({ type: 'folder', name: 'New folder', icon: 'folder-open', items: [] });

/** A click on the list: into the selected folder or empty slot, or the first empty slot. */
function quickAdd(p) {
  const S = slots(), make = () => (p.folder ? newFolder() : { type: 'action', ...structuredClone(p.item) });
  const fresh = !p.folder && p.item.command == null;
  if (sel) {
    const slot = sel.slot, s = S[slot];
    if (!p.folder && s.type === 'folder') {
      if (s.items.length >= FOLDER_MAX) { toast(`${s.name} is full: a folder holds up to ${FOLDER_MAX} actions`); return; }
      const at = sel.index != null ? sel.index + 1 : s.items.length;
      change(c => { c.ring.slots[slot].items.splice(at, 0, structuredClone(p.item)); });
      select({ slot, index: at }, fresh);
      if (!fresh) toast(`Added to ${s.name}`);
      return;
    }
    if (s.type === 'empty') { change(c => { c.ring.slots[slot] = make(); }); select({ slot }, fresh || p.folder); return; }
  }
  const free = S.findIndex(x => x.type === 'empty');
  if (free < 0) { toast('No empty slot. Drag it onto a slot to replace what is there, or into a folder.'); return; }
  change(c => { c.ring.slots[free] = make(); });
  select({ slot: free }, fresh || p.folder);
  if (!fresh && !p.folder) toast(`Added to the ${SLOT_POS[free]} slot`);
}

/* ---------- Drag and drop ---------- */

function dragPrepare(e, payload, onClick) {
  drag = { payload, onClick, x0: e.clientX, y0: e.clientY, started: false, ghost: null, plan: null, peekTimer: 0, peekSlot: null, id: e.pointerId };
  window.addEventListener('pointermove', dragMove);
  window.addEventListener('pointerup', dragEnd);
  window.addEventListener('pointercancel', dragCleanup);
}

function dragCleanup() {
  window.removeEventListener('pointermove', dragMove);
  window.removeEventListener('pointerup', dragEnd);
  window.removeEventListener('pointercancel', dragCleanup);
  if (drag) { clearTimeout(drag.peekTimer); drag.ghost?.remove(); }
  document.body.classList.remove('dragging');
  stage.setDrop(null, true);
  stage.unpeek();
  drag = null;
}

function payloadItem(p) {
  const S = slots();
  return p.kind === 'catalog' ? p.item : p.kind === 'slot' ? S[p.slot] : S[p.slot].items[p.index];
}

const isFolderPayload = p => (p.kind === 'catalog' && p.folder) || (p.kind === 'slot' && slots()[p.slot].type === 'folder');

function dragMove(e) {
  const d = drag;
  if (!d || e.pointerId !== d.id) return;
  if (!d.started) {
    if (Math.hypot(e.clientX - d.x0, e.clientY - d.y0) < 5) return;
    d.started = true;
    document.body.classList.add('dragging');
    d.ghost = document.createElement('div');
    d.ghost.className = 'dghost';
    d.ghost.innerHTML = `<span class="disc">${itemGlyph(payloadItem(d.payload))}</span><span class="tip" hidden></span>`;
    document.body.appendChild(d.ghost);
  }
  e.preventDefault();
  d.ghost.style.transform = `translate(${e.clientX - 23}px, ${e.clientY - 23}px)`;
  const t = stage.hitTestClient(e.clientX, e.clientY), p = d.payload, S = slots();
  // Resting on a closed folder opens it, to drop at the right place of its arc.
  const intoFolder = t.kind === 'folder' && !isFolderPayload(p) && !((p.kind === 'slot' || p.kind === 'child') && p.slot === t.slot);
  if (intoFolder && S[t.slot].items.length < FOLDER_MAX) {
    if (d.peekSlot !== t.slot) {
      clearTimeout(d.peekTimer);
      d.peekSlot = t.slot;
      d.peekTimer = setTimeout(() => stage.peekFolder(t.slot), 420);
    }
  } else if (!['child', 'ghost', 'gap'].includes(t.kind)) { clearTimeout(d.peekTimer); d.peekSlot = null; }
  d.plan = planDrop(p, t);
  stage.setDrop(d.plan ? t : null, !d.plan || d.plan.ok);
  const tip = $('.tip', d.ghost);
  tip.hidden = !d.plan;
  if (d.plan) { tip.textContent = d.plan.msg; tip.classList.toggle('bad', !d.plan.ok); }
}

function dragEnd(e) {
  const d = drag;
  if (!d || e.pointerId !== d.id) return;
  if (!d.started) { const fn = d.onClick; dragCleanup(); fn?.(); return; }
  const plan = d.plan;
  dragCleanup();
  if (plan?.ok) {
    change(s => plan.apply(s));
    select(plan.select, !!plan.focus);
    if (plan.done) toast(plan.done, { action: 'Undo', run: undoLast });
  } else if (plan) toast(plan.msg);
}

/** Takes the dragged item out of its place in settings `s`, as an action. */
function takeItem(s, p) {
  if (p.kind === 'catalog') return structuredClone(p.item);
  if (p.kind === 'slot') { const a = pickAction(s.ring.slots[p.slot]); s.ring.slots[p.slot] = { type: 'empty' }; return a; }
  return s.ring.slots[p.slot].items.splice(p.index, 1)[0];
}

/** What dropping `p` on target `t` does, or null when `t` takes nothing. */
function planDrop(p, t) {
  if (!t || !['empty', 'action', 'folder', 'child', 'ghost'].includes(t.kind)) return null;
  const S = slots(), folderP = isFolderPayload(p);
  const ok = (msg, apply, sel, extra = {}) => ({ ok: true, msg, apply, select: sel, ...extra });
  const no = msg => ({ ok: false, msg });
  const fresh = p.kind === 'catalog' && !p.folder && p.item.command == null;
  // Into a folder: its arc (an action or the +) or the folder itself.
  if (t.kind === 'child' || t.kind === 'ghost' || (t.kind === 'folder' && !folderP)) {
    if (folderP) return no('A folder can’t hold another folder');
    if (p.kind === 'slot' && p.slot === t.slot) return null;
    const f = S[t.slot], n = f.items.length;
    if (p.kind === 'child' && p.slot === t.slot) {
      if (t.kind === 'folder') return null;
      const to = t.kind === 'ghost' ? n - 1 : t.index;
      if (to === p.index) return null;
      return ok('Move here', s => { const items = s.ring.slots[t.slot].items; items.splice(to, 0, items.splice(p.index, 1)[0]); }, { slot: t.slot, index: to });
    }
    if (n >= FOLDER_MAX) return no(`${f.name} is full (${FOLDER_MAX} actions)`);
    const at = t.kind === 'child' ? t.index : n;
    return ok(`Add to ${f.name}`, s => { const it = takeItem(s, p); s.ring.slots[t.slot].items.splice(at, 0, it); }, { slot: t.slot, index: at }, { focus: fresh });
  }
  // Onto a slot of the ring.
  if (p.kind === 'slot' && p.slot === t.slot) return null;
  if (p.kind === 'catalog') {
    if (p.folder) {
      if (t.kind === 'empty') return ok('New folder here', s => { s.ring.slots[t.slot] = newFolder(); }, { slot: t.slot }, { focus: true });
      if (t.kind === 'action') {
        return ok(`Put “${S[t.slot].name}” in a new folder`, s => {
          s.ring.slots[t.slot] = { ...newFolder(), items: [pickAction(s.ring.slots[t.slot])] };
        }, { slot: t.slot }, { focus: true });
      }
      return no('A folder can’t hold another folder');
    }
    if (t.kind === 'empty') return ok('Add here', s => { s.ring.slots[t.slot] = { type: 'action', ...structuredClone(p.item) }; }, { slot: t.slot }, { focus: fresh });
    return ok(`Replace “${S[t.slot].name}”`, s => { s.ring.slots[t.slot] = { type: 'action', ...structuredClone(p.item) }; }, { slot: t.slot }, { focus: fresh, done: `Replaced “${S[t.slot].name}”` });
  }
  if (p.kind === 'slot') {
    return ok(t.kind === 'empty' ? 'Move here' : `Swap with “${S[t.slot].name}”`, s => {
      const a = s.ring.slots[p.slot];
      s.ring.slots[p.slot] = s.ring.slots[t.slot];
      s.ring.slots[t.slot] = a;
    }, { slot: t.slot });
  }
  if (p.kind === 'child') {
    if (t.kind === 'empty') {
      return ok('Move out of the folder', s => { const it = s.ring.slots[p.slot].items.splice(p.index, 1)[0]; s.ring.slots[t.slot] = { type: 'action', ...it }; }, { slot: t.slot });
    }
    if (t.kind === 'action') {
      return ok(`Swap with “${S[t.slot].name}”`, s => {
        const f = s.ring.slots[p.slot], it = f.items[p.index];
        f.items[p.index] = pickAction(s.ring.slots[t.slot]);
        s.ring.slots[t.slot] = { type: 'action', ...it };
      }, { slot: t.slot });
    }
  }
  return null;
}
