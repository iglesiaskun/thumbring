// Animation: how the ring moves, with a live preview of your own ring.
//
// It changes only the looks, never what the buttons do. Releasing a slider
// plays again the animation it changes.

import { $, esc, icon as drawIcon, sliderRow, toast } from '../ui.js';
import { change, get, set, store, undo } from '../store.js';
import { BUTTON_NAMES } from '../commands.js';
import { createRingStage } from '../ring-stage.js';

export const id = 'animation';
export const title = 'Animation';
export const icon = 'sparkles';
export const icons = ['play', 'rotate-ccw'];

/** `MouseEvent.button` of each button the ring can use. */
const BUTTON_CODES = { back: 3, forward: 4, middle: 1 };
const ms = v => `${v} ms`;
/** Each slider: its label, step, format, note, and the animation it plays again. */
const SLIDERS = {
  'animation.speed': { label: 'Overall speed', step: 0.1, format: v => `${Number(v).toFixed(1)}×`, note: 'Speeds up or slows down every animation at once. The hold time and the path tolerance stay as they are.', replay: 'ring' },
  'animation.ring_open_ms': { label: 'Ring opens', step: 10, format: ms, replay: 'ring' },
  'animation.ring_close_ms': { label: 'Ring closes', step: 10, format: ms, replay: 'ring' },
  'animation.blob_ms_per_item': { label: 'Blob travel', step: 5, format: v => `${v} ms per item`, note: 'Time for each item width the blob travels from the folder to its first action.', replay: 'folder' },
  'animation.folder_items_ms': { label: 'Folder actions appear', step: 10, format: ms, replay: 'folder' },
  'animation.folder_close_ms': { label: 'Folder closes', step: 10, format: ms, replay: 'folder' },
  'animation.viscosity': { label: 'Viscosity', step: 0.05, format: v => Number(v).toFixed(2), note: 'How thick the neck is and how far the blob goes before it lets go.', replay: 'folder' },
  'animation.break_at': { label: 'Break point', step: 0.01, format: v => `${Math.round(v * 100)}% of the trip`, note: 'When the neck breaks and the blob is free.', replay: 'folder' },
  'animation.wobble': { label: 'Wobble', step: 0.05, format: v => Number(v).toFixed(2), note: '0 lands still; higher overshoots and wobbles like jelly.', replay: 'folder' },
};
const TIMINGS = ['animation.ring_open_ms', 'animation.ring_close_ms', 'animation.blob_ms_per_item', 'animation.folder_items_ms', 'animation.folder_close_ms'];
const SHAPE = ['animation.viscosity', 'animation.break_at', 'animation.wobble'];

let root = null, stage = null, slow = false;
const reducedByWindows = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

function slider(path) {
  const [min, max] = store.limits[path];
  return sliderRow({ path, min, max, value: get(store.settings, path), defaultValue: get(store.defaults, path), ...SLIDERS[path] });
}

export function mount(pageRoot) {
  root = pageRoot;
  root.innerHTML = `
    <header class="ph"><h1>${title}</h1><p>How the ring moves. It changes only the looks, never what the buttons do.</p></header>
    <div class="a-layout">
      <div class="a-stage-col">
        <div class="stage demo" id="demoStage" aria-label="Animation preview with your ring">
          <canvas></canvas>
          <div class="layer overlay"></div>
          <div class="layer icons"></div>
          <div class="label"><span class="nm"></span><small class="k"></small></div>
          <p class="hint" id="demo-hint"></p>
          <svg class="fake-cursor" viewBox="0 0 20 28" aria-hidden="true"><path d="M2 2 L2 22 L7.4 16.8 L10.9 25 L14.4 23.5 L11 15.4 L18 15.4 Z"/></svg>
          <div class="stoast" role="status"><span class="t"></span><span class="tag"></span></div>
        </div>
        <div class="stage-tools">
          <button type="button" class="btn" id="demo-play">${drawIcon('play')}Play demo</button>
          <button type="button" class="btn" id="demo-replay">${drawIcon('rotate-ccw')}Replay opening</button>
          <label class="tgl" for="demo-slow"><input type="checkbox" class="switch" id="demo-slow">Slow motion ×5</label>
        </div>
      </div>
      <div class="stack" id="anim-controls"></div>
    </div>`;

  stage = createRingStage($('#demoStage', root), {
    mode: 'demo',
    getSlots: () => store.settings.ring.slots,
    getAnim: () => store.settings.animation,
    getBehavior: () => {
      const b = store.settings.behavior;
      return { openWith: b.open_with, executeOnRelease: b.execute_on_release, holdMs: b.hold_to_open_ms, intentMs: b.path_tolerance_ms };
    },
    slowmo: () => slow,
    reduced: () => reducedByWindows() && !store.settings.animation.ignore_system_reduced_motion,
    active: () => !root.hidden,
    blocked: () => !$('#scrim').hidden,
    triggerButton: () => BUTTON_CODES[store.settings.ring.button],
    triggerName: () => BUTTON_NAMES[store.settings.ring.button],
    autoReopen: true,
  });

  $('#demo-play', root).addEventListener('click', () => stage.playDemo());
  $('#demo-replay', root).addEventListener('click', () => stage.replay('ring'));
  $('#demo-slow', root).addEventListener('change', e => { slow = e.target.checked; });

  const controls = $('#anim-controls', root);
  controls.addEventListener('input', e => {
    const path = e.target.dataset.path;
    if (!path || !SLIDERS[path]) return;
    const value = Number(e.target.value);
    change(s => set(s, path, value), { key: path, render: false });
    const out = $(`#${e.target.id}-out`, root);
    out.textContent = SLIDERS[path].format(value);
    out.classList.toggle('changed', value !== get(store.defaults, path));
  });
  // A released slider plays its animation again.
  controls.addEventListener('change', e => {
    const path = e.target.dataset.path;
    if (path && SLIDERS[path]) { stage.replay(SLIDERS[path].replay); return; }
    if (e.target.name === 'blob-style') {
      const style = e.target.value;
      change(s => { Object.assign(s.animation, { style }, store.blobPresets[style]); });
      toast(`${style === 'viscous' ? 'Viscous' : 'Elastic'}: the blob shape went to the values of this style`);
      stage.replay('folder');
      $(`[name="blob-style"][value="${style}"]`, root)?.focus();
    } else if (e.target.id === 'full-motion') {
      const value = e.target.checked;
      change(s => { s.animation.ignore_system_reduced_motion = value; });
      $('#full-motion', root)?.focus();
    }
  });
  controls.addEventListener('click', e => {
    if (!e.target.closest('#anim-reset')) return;
    change(s => { s.animation = structuredClone(store.defaults.animation); });
    toast('The default animation is back', { action: 'Undo', run: undo });
    stage.replay('ring');
  });
}

export function render() {
  const a = store.settings.animation, tap = store.settings.behavior.open_with === 'tap';
  const button = esc(BUTTON_NAMES[store.settings.ring.button]);
  $('#demo-hint', root).innerHTML = tap
    ? `Tap <kbd>${button}</kbd> or <kbd>Space</kbd> over the preview to open the ring there · <kbd>Esc</kbd> closes it`
    : `Point at a folder, click an action · hold <kbd>${button}</kbd> or <kbd>Space</kbd> to open the ring under the cursor`;
  const isDefault = JSON.stringify(a) === JSON.stringify(store.defaults.animation);
  $('#anim-controls', root).innerHTML = `
    <div class="card">
      <div class="sub-h">Blob style</div>
      <div class="style-cards" role="radiogroup" aria-label="Blob style">
        <label class="style-card"><input type="radio" name="blob-style" value="viscous" ${a.style === 'viscous' ? 'checked' : ''}><b>Viscous</b><small>Thick neck that breaks at the end. The default.</small></label>
        <label class="style-card"><input type="radio" name="blob-style" value="elastic" ${a.style === 'elastic' ? 'checked' : ''}><b>Elastic</b><small>Lets go early, flies and wobbles.</small></label>
      </div>
      ${slider('animation.speed')}
    </div>
    <div class="card"><div class="sub-h">Timings</div>${TIMINGS.map(slider).join('')}</div>
    <div class="card"><div class="sub-h">Blob shape</div>${SHAPE.map(slider).join('')}</div>
    <div class="card"><div class="row">
      <div class="row-text"><label class="row-title" for="full-motion">Animate fully even with Windows animation effects off</label>
        <span class="row-desc">${reducedByWindows()
          ? 'Animation effects are off in Windows right now, so the ring plays every animation at 35% of these times.'
          : 'With animation effects off in Windows, the ring plays every animation at 35% of these times.'}</span></div>
      <input type="checkbox" class="switch" id="full-motion" ${a.ignore_system_reduced_motion ? 'checked' : ''}>
    </div></div>
    <button type="button" class="btn start" id="anim-reset" ${isDefault ? 'disabled' : ''}>${drawIcon('rotate-ccw')}Restore the default animation</button>`;
  stage.rebuild();
}

/** The page is on screen: the ring opens in the preview. */
export function shown() {
  stage.show();
}
