// Behavior: which mouse buttons Thumbring uses, how the ring opens and the
// timings of the ring's logic.

import { $, esc, icon as drawIcon, openDialog, sliderRow, toast } from '../ui.js';
import { change, get, set, store } from '../store.js';
import { BUTTON_NAMES } from '../commands.js';

export const id = 'behavior';
export const title = 'Behavior';
export const icon = 'mouse-pointer-click';
export const icons = ['move', 'circle-dot', 'mouse'];

/** `MouseEvent.button` to the names in the settings file. */
const PRESSED = { 0: 'left', 1: 'middle', 2: 'right', 3: 'back', 4: 'forward' };
const USES = { gestures: 'gestures', ring: 'the ring' };

const SLIDERS = {
  'behavior.hold_to_open_ms': {
    label: 'Hold time',
    step: 10,
    format: v => `${v} ms`,
    note: 'How long to hold the button before the ring opens. Releasing sooner, without dragging, counts as a quick tap, for gestures too.',
  },
  'behavior.path_tolerance_ms': {
    label: 'Path tolerance',
    step: 10,
    format: v => `${v} ms`,
    note: 'With a folder open, the cursor has to rest this long on another item before it takes the highlight, so crossing the ring on the way to a far action keeps the folder open.',
  },
};

function slider(path) {
  const [min, max] = store.limits[path];
  return sliderRow({ path, min, max, value: get(store.settings, path), defaultValue: get(store.defaults, path), ...SLIDERS[path] });
}

const buttonOf = use => (use === 'ring' ? store.settings.ring.button : store.settings.gestures.button);

function buttonRow(use, rowIcon, rowTitle, desc) {
  return `<div class="row">
    <span class="row-ic">${drawIcon(rowIcon)}</span>
    <div class="row-text"><span class="row-title">${rowTitle}</span><span class="row-desc">${desc}</span></div>
    <span class="btnchip">${drawIcon('mouse')}${esc(BUTTON_NAMES[buttonOf(use)])}</span>
    <button type="button" class="btn" data-choose="${use}">Change</button>
  </div>`;
}

export function render(root) {
  const b = store.settings.behavior, tap = b.open_with === 'tap';
  root.innerHTML = `
    <header class="ph"><h1>${title}</h1><p>Which buttons Thumbring uses and how the ring responds to them.</p></header>
    <h2 class="sect">Mouse buttons</h2>
    <div class="card">
      ${buttonRow('gestures', 'move', 'Gestures', 'Hold it and drag.')}
      ${buttonRow('ring', 'circle-dot', 'Ring', 'Hold it to open the ring.')}
    </div>
    <h2 class="sect">Opening the ring</h2>
    <div class="card">
      <div class="row">
        <div class="row-text"><span class="row-title" id="lbl-open-with">Open the ring by</span>
          <span class="row-desc">${tap
            ? 'Tapping the button; another tap closes it. The button loses its usual function.'
            : 'Holding the button. A quick tap still does what the button always did.'}</span></div>
        <div class="seg" role="radiogroup" aria-labelledby="lbl-open-with">
          <input type="radio" name="open-with" id="ow-hold" value="hold" ${tap ? '' : 'checked'}><label for="ow-hold">Holding</label>
          <input type="radio" name="open-with" id="ow-tap" value="tap" ${tap ? 'checked' : ''}><label for="ow-tap">Tapping</label>
        </div>
      </div>
      <div class="row">
        <div class="row-text"><label class="row-title" for="quick">Run an action when releasing the button over it</label>
          <span class="row-desc">${tap
            ? 'Only when the ring opens by holding.'
            : 'Quick mode: hold, point and release. Releasing over an action runs it, and on the center closes the ring. Off, the ring stays open wherever you release the button, the center too, and an action takes a click.'}</span></div>
        <input type="checkbox" class="switch" id="quick" ${b.execute_on_release ? 'checked' : ''} ${tap ? 'disabled' : ''}>
      </div>
      ${slider('behavior.hold_to_open_ms')}
      <div class="row">
        <div class="row-text"><span class="row-title">Feel the hold time</span>
          <span class="row-desc">Press and hold the box with any mouse button.</span></div>
        <div class="holdpad" id="holdpad" role="button" tabindex="0" aria-label="Hold to try the hold time"><span class="fill"></span><span class="hp-t">Hold here</span></div>
      </div>
    </div>
    <h2 class="sect">Inside the ring</h2>
    <div class="card">${slider('behavior.path_tolerance_ms')}</div>`;
  bindHoldPad($('#holdpad', root));
}

export function mount(root) {
  root.addEventListener('change', e => {
    const t = e.target;
    if (t.name === 'open-with') {
      const value = t.value;
      change(s => { s.behavior.open_with = value; });
      $(`#ow-${value}`, root)?.focus();
    } else if (t.id === 'quick') {
      const value = t.checked;
      change(s => { s.behavior.execute_on_release = value; });
      $('#quick', root)?.focus();
    }
  });
  root.addEventListener('input', e => {
    const path = e.target.dataset.path;
    if (!path || !SLIDERS[path]) return;
    const value = Number(e.target.value);
    change(s => set(s, path, value), { key: path, render: false });
    const out = $(`#${e.target.id}-out`, root);
    out.textContent = SLIDERS[path].format(value);
    out.classList.toggle('changed', value !== get(store.defaults, path));
  });
  root.addEventListener('click', e => {
    const b = e.target.closest('[data-choose]');
    if (b) chooseButton(b.dataset.choose);
  });
}

/* ---------- Choosing a button by pressing it ---------- */

function mouseDrawing(hit, bad) {
  const cls = b => (hit === b ? ' m-hit' : bad === b ? ' m-bad' : '');
  return `<svg viewBox="0 0 120 180" aria-hidden="true">
    <path class="m-body" d="M60 8 C 92 8 108 34 108 74 L108 122 C108 154 88 172 60 172 C32 172 12 154 12 122 L12 74 C12 34 28 8 60 8 Z"/>
    <path class="m-zone${cls('left')}" d="M60 9 C 30 9 13 34 13 74 L60 74 Z"/>
    <path class="m-zone${cls('right')}" d="M60 9 C 90 9 107 34 107 74 L60 74 Z"/>
    <path class="m-line" d="M60 8 V 74 M12 74 H 108"/>
    <rect class="m-btn${cls('middle')}" x="53" y="24" width="14" height="28" rx="7"/>
    <rect class="m-btn${cls('forward')}" x="3" y="80" width="11" height="24" rx="4"/>
    <rect class="m-btn${cls('back')}" x="3" y="108" width="11" height="24" rx="4"/>
  </svg>`;
}

function chooseButton(use) {
  const other = use === 'ring' ? 'gestures' : 'ring';
  const state = { message: null, detail: null, hit: null, bad: null, conflict: null };
  const body = () => {
    const big = state.conflict
      ? `${BUTTON_NAMES[state.conflict]} already runs ${USES[other]}`
      : state.message ?? 'Press the mouse button you want to use';
    const small = state.conflict
      ? `Swap them? Then ${USES[other]} would use ${BUTTON_NAMES[buttonOf(use)]}.`
      : state.detail ?? 'Back, Forward or the middle button (the wheel). Press it over this box.';
    return `<div class="cap-pad" id="cap-pad">${mouseDrawing(state.hit, state.bad)}
      <div class="cap-text"><p class="cap-big ${state.bad ? 'bad' : ''}" aria-live="polite">${esc(big)}</p><p>${esc(small)}</p></div></div>`;
  };
  const cancel = [{ label: 'Cancel', value: 'cancel' }];
  const dialog = openDialog({
    title: use === 'ring' ? 'Choose the button for the ring' : 'Choose the button for gestures',
    body: body(),
    buttons: cancel,
  });
  const show = (changes, buttons = cancel) => {
    Object.assign(state, { message: null, detail: null, hit: null, bad: null, conflict: null }, changes);
    dialog.update({ body: body(), buttons });
  };

  const onPress = e => {
    if (!e.target.closest('#cap-pad')) return;
    e.preventDefault();
    const pressed = PRESSED[e.button];
    if (!pressed) return;
    if (pressed === 'left' || pressed === 'right') {
      show({ message: `The ${pressed} button can’t be used`, detail: 'You need it for everything else. Use Back, Forward or the middle button.', bad: pressed });
    } else if (pressed === buttonOf(use)) {
      dialog.close('same');
      toast(`${BUTTON_NAMES[pressed]} already runs ${USES[use]}`);
    } else if (pressed === buttonOf(other)) {
      show({ conflict: pressed, hit: pressed }, [...cancel, { label: 'Swap them', primary: true, value: 'swap' }]);
    } else {
      dialog.close('chosen');
      change(s => { s[use].button = pressed; });
      toast(`${use === 'ring' ? 'The ring now uses' : 'Gestures now use'} ${BUTTON_NAMES[pressed]}`);
    }
  };
  // While choosing, clicks on the drawing are only for choosing.
  const block = e => { if (e.target.closest('#cap-pad')) e.preventDefault(); };
  dialog.root.addEventListener('mousedown', onPress);
  dialog.root.addEventListener('auxclick', block);
  dialog.root.addEventListener('contextmenu', block);

  dialog.done.then(result => {
    if (result !== 'swap') return;
    const mine = buttonOf(use), theirs = state.conflict;
    change(s => { s[other].button = mine; s[use].button = theirs; });
    toast(`Swapped: ${use === 'ring' ? 'the ring now uses' : 'gestures now use'} ${BUTTON_NAMES[theirs]}`);
  });
}

/* ---------- The box that fills up for as long as the hold time ---------- */

function bindHoldPad(pad) {
  const label = $('.hp-t', pad);
  let timer = 0, down = false, opened = false, resetTimer = 0;
  const start = () => {
    const { hold_to_open_ms: ms, open_with: openWith } = store.settings.behavior;
    clearTimeout(resetTimer);
    down = true;
    opened = false;
    pad.classList.remove('opened', 'holding');
    void pad.offsetWidth;
    if (openWith === 'tap') {
      opened = true;
      pad.classList.add('opened');
      label.textContent = 'The ring opens';
      return;
    }
    pad.style.setProperty('--hold', ms + 'ms');
    pad.classList.add('holding');
    label.textContent = 'Holding…';
    timer = setTimeout(() => {
      opened = true;
      pad.classList.add('opened');
      label.textContent = 'The ring opens';
    }, ms);
  };
  const end = () => {
    if (!down) return;
    down = false;
    clearTimeout(timer);
    if (!opened) {
      pad.classList.remove('holding');
      label.textContent = `Quick tap: ${BUTTON_NAMES[store.settings.ring.button]} as usual`;
    }
    resetTimer = setTimeout(() => {
      pad.classList.remove('opened', 'holding');
      label.textContent = 'Hold here';
    }, 1400);
  };
  pad.addEventListener('pointerdown', e => {
    e.preventDefault();
    try { pad.setPointerCapture(e.pointerId); } catch { /* no capture */ }
    start();
  });
  pad.addEventListener('pointerup', end);
  pad.addEventListener('pointercancel', end);
  pad.addEventListener('contextmenu', e => e.preventDefault());
  pad.addEventListener('keydown', e => {
    if ((e.key === ' ' || e.key === 'Enter') && !e.repeat && !down) { e.preventDefault(); start(); }
  });
  pad.addEventListener('keyup', e => { if (e.key === ' ' || e.key === 'Enter') end(); });
}
