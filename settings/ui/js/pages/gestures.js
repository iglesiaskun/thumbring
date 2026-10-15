// Gestures: what each direction and the quick tap do, how far to drag, and
// a box to try a gesture.

import { $, esc, goto, icon as drawIcon, sliderRow } from '../ui.js';
import { change, get, set, store } from '../store.js';
import { BUTTON_NAMES, COMMAND_ICONS, commandHTML } from '../commands.js';
import { BUILDER_ICONS, chooseCommand } from '../command-builder.js';
import { browse } from '../system.js';

export const id = 'gestures';
export const title = 'Gestures';
export const icon = 'move';
export const icons = [...COMMAND_ICONS, ...BUILDER_ICONS, 'arrow-up', 'arrow-down', 'arrow-left', 'arrow-right', 'mouse-pointer-click', 'undo-2', 'hand'];

export const GESTURES = ['up', 'down', 'left', 'right', 'click'];
export const GESTURE_LABELS = { up: 'Drag up', down: 'Drag down', left: 'Drag left', right: 'Drag right', click: 'Quick tap' };
export const GESTURE_ICONS = { up: 'arrow-up', down: 'arrow-down', left: 'arrow-left', right: 'arrow-right', click: 'mouse-pointer-click' };
/** `MouseEvent.button` of each button the gestures can use. */
const BUTTON_CODES = { back: 3, forward: 4, middle: 1 };
const THRESHOLD = 'gestures.threshold_px';

let root = null;

export function mount(pageRoot) {
  root = pageRoot;
  root.innerHTML = `
    <header class="ph"><h1>${title}</h1><p>Hold <span id="g-button">Back</span>, drag and release. A quick tap still does what the button always did.</p></header>
    <div class="g-layout">
      <div class="stack">
        <div class="card compass" id="compass"></div>
        <div class="card" id="g-threshold"></div>
        <p class="muted small">Apps can have gestures of their own: <button type="button" class="linkbtn" id="to-profiles">profiles, in Apps</button>.</p>
      </div>
      <div class="card trycard">
        <h2>Try it</h2>
        <p>Hold the left button (or <span class="g-button">Back</span>) in the box, drag and release.</p>
        <div class="trypad" id="trypad"><svg aria-hidden="true"></svg><div class="pad-hint" id="pad-hint">Press, drag past the circle and release</div></div>
        <div class="try-result" id="try-result" aria-live="polite"><span class="muted">The result shows up here.</span></div>
      </div>
    </div>`;

  $('#compass', root).addEventListener('click', async e => {
    const b = e.target.closest('[data-gesture]');
    if (!b) return;
    const k = b.dataset.gesture;
    const value = await chooseCommand({
      title: k === 'click' ? 'What does a quick tap do?' : `What does “${GESTURE_LABELS[k].toLowerCase()}” do?`,
      value: store.settings.gestures[k], mode: 'gesture', ctx: k === 'click' ? 'tap' : 'gesture', browse,
    });
    if (value !== undefined) change(s => { s.gestures[k] = value; });
  });
  root.addEventListener('input', e => {
    if (e.target.dataset.path !== THRESHOLD) return;
    const value = Number(e.target.value);
    change(s => set(s, THRESHOLD, value), { key: THRESHOLD, render: false });
    const out = $(`#${e.target.id}-out`, root);
    out.textContent = `${value} px`;
    out.classList.toggle('changed', value !== get(store.defaults, THRESHOLD));
  });
  $('#to-profiles', root).addEventListener('click', () => goto('apps'));
  bindTryPad($('#trypad', root));
}

export function render() {
  const button = BUTTON_NAMES[store.settings.gestures.button];
  $('#g-button', root).textContent = button;
  $('.g-button', root).textContent = button;
  $('#compass', root).innerHTML = GESTURES.map(k =>
    `<button type="button" class="gcard g-${k}" data-gesture="${k}">
      <span class="gdir">${drawIcon(GESTURE_ICONS[k])}${GESTURE_LABELS[k]}</span>
      ${commandHTML(store.settings.gestures[k], k === 'click' ? 'tap' : 'gesture')}
    </button>`).join('');
  const [min, max] = store.limits[THRESHOLD];
  $('#g-threshold', root).innerHTML = sliderRow({
    path: THRESHOLD, label: 'Drag distance', min, max, step: 1,
    value: get(store.settings, THRESHOLD), defaultValue: get(store.defaults, THRESHOLD),
    format: v => `${v} px`,
    note: 'How far to drag before it counts as a gesture, at 100% scaling. On screens with a larger scale, Thumbring grows it in the same proportion.',
  });
}

/* ---------- Try it: the same rules as the engine (core/src/gesture.rs) ---------- */

function bindTryPad(pad) {
  const drawing = $('svg', pad);
  let g = null;
  const draw = () => {
    const threshold = store.settings.gestures.threshold_px;
    const points = g.points.map(p => p.map(v => v.toFixed(1)).join(',')).join(' ');
    drawing.innerHTML = `<circle class="thr" cx="${g.x0}" cy="${g.y0}" r="${threshold}"/><polyline class="trail" points="${points}"/><circle class="org" cx="${g.x0}" cy="${g.y0}" r="3.5"/>`;
  };
  pad.addEventListener('pointerdown', e => {
    if (e.button !== 0 && e.button !== BUTTON_CODES[store.settings.gestures.button]) return;
    e.preventDefault();
    try { pad.setPointerCapture(e.pointerId); } catch { /* no capture */ }
    const r = pad.getBoundingClientRect();
    g = { id: e.pointerId, x0: e.clientX - r.left, y0: e.clientY - r.top, t0: performance.now(), points: [], exceeded: false };
    g.points.push([g.x0, g.y0]);
    $('#pad-hint', root).hidden = true;
    draw();
  });
  pad.addEventListener('pointermove', e => {
    if (!g || e.pointerId !== g.id) return;
    const r = pad.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top;
    g.points.push([x, y]);
    if (Math.hypot(x - g.x0, y - g.y0) >= store.settings.gestures.threshold_px) g.exceeded = true;
    draw();
  });
  pad.addEventListener('pointerup', e => {
    if (!g || e.pointerId !== g.id) return;
    const [x, y] = g.points[g.points.length - 1], dx = x - g.x0, dy = y - g.y0;
    const held = performance.now() - g.t0, result = $('#try-result', root);
    let k = null;
    // Past the threshold, the main axis wins; on the exact diagonal, the vertical one.
    if (Math.hypot(dx, dy) >= store.settings.gestures.threshold_px) k = Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? 'right' : 'left') : dy > 0 ? 'down' : 'up';
    else if (g.exceeded) result.innerHTML = `${drawIcon('undo-2')}<span>Went past the circle and came back: cancelled, nothing happens</span>`;
    else if (held < store.settings.behavior.hold_to_open_ms) k = 'click';
    else result.innerHTML = `${drawIcon('hand')}<span>Held still for ${Math.round(held)} ms: nothing happens</span>`;
    if (k) {
      result.innerHTML = `${drawIcon(GESTURE_ICONS[k])}<span><b>${esc(GESTURE_LABELS[k])}</b></span>${drawIcon('arrow-right')}${commandHTML(store.settings.gestures[k], k === 'click' ? 'tap' : 'gesture')}`;
      const card = $(`#compass [data-gesture="${k}"]`, root);
      card.classList.remove('flash');
      void card.offsetWidth;
      card.classList.add('flash');
    }
    g = null;
  });
  pad.addEventListener('pointercancel', () => { g = null; drawing.innerHTML = ''; });
  pad.addEventListener('contextmenu', e => e.preventDefault());
}
