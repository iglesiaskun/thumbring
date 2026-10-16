// Appearance: colors, transparency and size of the ring, with a live
// preview of your own ring and a button that shows it on the screen.
//
// Like the animation, it changes only the looks. The icon of each item is
// chosen in Ring.

import { $, $$, esc, goto, icon as drawIcon, sliderRow, toast } from '../ui.js';
import { change, get, set, store, undo } from '../store.js';
import { BUTTON_NAMES } from '../commands.js';
import { createRingStage, defaultRadius } from '../ring-stage.js';
import { invoke } from '../system.js';

export const id = 'appearance';
export const title = 'Appearance';
export const icon = 'palette';
export const icons = ['monitor', 'image', 'rotate-ccw'];

/** `MouseEvent.button` of each button the ring can use. */
const BUTTON_CODES = { back: 3, forward: 4, middle: 1 };

/** Ready-made looks. They set the colors, the opacity, the shadow and the label, never the size. */
const PRESETS = {
  classic: { name: 'Classic', desc: 'White items, black highlight', item_color: '#FFFFFF', item_opacity: 0.96, icon_color: '#2A2B31', highlight_color: '#121214', highlight_icon_color: '#FFFFFF', cancel_color: '#E64054', shadow: true, label: 'light' },
  midnight: { name: 'Midnight', desc: 'Dark items, white highlight', item_color: '#1F2026', item_opacity: 0.94, icon_color: '#E8E9EE', highlight_color: '#FFFFFF', highlight_icon_color: '#16171B', cancel_color: '#FF5A6E', shadow: true, label: 'dark' },
  glass: { name: 'Glass', desc: 'See-through items', item_color: '#FFFFFF', item_opacity: 0.55, icon_color: '#16171B', highlight_color: '#121214', highlight_icon_color: '#FFFFFF', cancel_color: '#E64054', shadow: false, label: 'light' },
  accent: { name: 'Accent', desc: 'Blue highlight', item_color: '#FFFFFF', item_opacity: 0.96, icon_color: '#2A2B31', highlight_color: '#2F5BD3', highlight_icon_color: '#FFFFFF', cancel_color: '#E64054', shadow: true, label: 'light' },
};
const presetValues = key => { const { name, desc, ...values } = PRESETS[key]; return values; };
const PRESET_KEYS = Object.keys(presetValues('classic'));
const presetOf = a => Object.keys(PRESETS).find(k => PRESET_KEYS.every(f => presetValues(k)[f] === a[f])) ?? null;

const COLORS = [
  ['item_color', 'Items', 'The circles of the ring and the center button.'],
  ['icon_color', 'Icons', 'On the items without the highlight.'],
  ['highlight_color', 'Highlight', 'The item under the cursor, the open folder and the blob.'],
  ['highlight_icon_color', 'Icon on the highlight', ''],
  ['cancel_color', 'Cancel', 'The center button under the cursor: a click there closes the ring, and so does releasing there in quick mode.'],
];

const OPACITY = 'appearance.item_opacity', SIZE = 'appearance.item_size_px', RADIUS = 'appearance.ring_radius_px';

let root = null, stage = null;

const look = () => store.settings.appearance;
/** The radius the engine uses: the file's own, or 1.8 items. */
const radius = a => a.ring_radius_px ?? defaultRadius(a.item_size_px);
/** The radius follows the item size: the file sets none, or exactly that one. */
const automatic = a => a.ring_radius_px == null || Math.abs(a.ring_radius_px - defaultRadius(a.item_size_px)) < 0.05;
/** The radii allowed for an item size, rounded inwards to tenths (Appearance::ring_radius_px_range). */
function radiusRange(size) {
  const [min, max] = store.limits['appearance.ring_radius_per_item'];
  return [Math.ceil(size * min * 10 - 1e-9) / 10, Math.floor(size * max * 10 + 1e-9) / 10];
}
const clamp = (v, [min, max]) => Math.min(max, Math.max(min, v));

/* ---------- Contrast, as WCAG measures it ---------- */

const channels = hex => { const n = parseInt(hex.slice(1), 16); return [n >> 16 & 255, n >> 8 & 255, n & 255].map(v => v / 255); };
const luminance = hex => channels(hex).map(v => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)).reduce((s, v, i) => s + v * [0.2126, 0.7152, 0.0722][i], 0);
const contrast = (a, b) => { const x = luminance(a), y = luminance(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };

function lookWarnings(a) {
  const w = [];
  if (contrast(a.icon_color, a.item_color) < 3) w.push('The icons are hard to see on the items.');
  if (contrast(a.highlight_icon_color, a.highlight_color) < 3) w.push('The icon on the highlight is hard to see.');
  if (contrast(a.highlight_color, a.item_color) < 1.6) w.push('The highlight looks almost like the other items.');
  return w;
}

/* ---------- Page ---------- */

export function mount(pageRoot) {
  root = pageRoot;
  root.innerHTML = `
    <header class="ph ph-row"><div><h1>${title}</h1><p>Colors, transparency and size of the ring. Each item’s icon is chosen in Ring.</p></div>
      <button type="button" class="btn" id="look-screen">${drawIcon('monitor')}Show on my screen</button></header>
    <div class="a-layout">
      <div class="a-stage-col">
        <div class="stage demo" id="lookStage" aria-label="Appearance preview with your ring">
          <canvas></canvas>
          <div class="layer overlay"></div>
          <div class="layer icons"></div>
          <div class="label"><span class="nm"></span><small class="k"></small></div>
          <p class="hint" id="look-hint"></p>
          <svg class="fake-cursor" viewBox="0 0 20 28" aria-hidden="true"><path d="M2 2 L2 22 L7.4 16.8 L10.9 25 L14.4 23.5 L11 15.4 L18 15.4 Z"/></svg>
          <div class="stoast" role="status"><span class="t"></span><span class="tag"></span></div>
        </div>
        <div class="stage-tools">
          <span class="muted small">Wallpaper</span>
          <div class="seg" role="radiogroup" aria-label="Preview wallpaper">
            <input type="radio" name="look-wall" id="lw-theme" value="theme" checked><label for="lw-theme">Theme</label>
            <input type="radio" name="look-wall" id="lw-light" value="light"><label for="lw-light">Light</label>
            <input type="radio" name="look-wall" id="lw-dark" value="dark"><label for="lw-dark">Dark</label>
          </div>
          <span class="muted small" id="look-fit" hidden></span>
        </div>
      </div>
      <div class="stack" id="look-controls"></div>
    </div>`;

  stage = createRingStage($('#lookStage', root), {
    mode: 'demo',
    getSlots: () => store.settings.ring.slots,
    getAnim: () => store.settings.animation,
    getLook: look,
    getBehavior: () => {
      const b = store.settings.behavior;
      return { openWith: b.open_with, executeOnRelease: b.execute_on_release, holdMs: b.hold_to_open_ms, intentMs: b.path_tolerance_ms };
    },
    reduced: () => matchMedia('(prefers-reduced-motion: reduce)').matches && !store.settings.animation.ignore_system_reduced_motion,
    active: () => !root.hidden,
    blocked: () => !$('#scrim').hidden,
    triggerButton: () => BUTTON_CODES[store.settings.ring.button],
    triggerName: () => BUTTON_NAMES[store.settings.ring.button],
    autoReopen: true,
    onFit: ratio => {
      const note = $('#look-fit', root);
      if (!note) return;
      note.hidden = ratio > 0.999;
      note.textContent = `Shown at ${Math.round(ratio * 100)}% to fit`;
    },
  });

  for (const r of $$('[name="look-wall"]', root)) {
    r.addEventListener('change', () => {
      const st = $('#lookStage', root);
      st.classList.toggle('wall-light', r.value === 'light');
      st.classList.toggle('wall-dark', r.value === 'dark');
    });
  }
  $('#look-screen', root).addEventListener('click', showOnScreen);

  const controls = $('#look-controls', root);
  controls.addEventListener('input', onInput);
  controls.addEventListener('change', onChangeEvent);
  controls.addEventListener('click', onClick);
}

export function render() {
  const a = look(), preset = presetOf(a), button = esc(BUTTON_NAMES[store.settings.ring.button]);
  $('#look-hint', root).innerHTML = `Point at a folder or an action to see the highlight · hold <kbd>${button}</kbd> or <kbd>Space</kbd> to open the ring under the cursor`;
  $('#look-controls', root).innerHTML = `
    <div class="card"><div class="sub-h">Presets</div>
      <div class="presets">${Object.entries(PRESETS).map(([k, p]) => `<button type="button" class="preset" data-preset="${k}" aria-pressed="${preset === k}"><span class="swatch" aria-hidden="true">${swatch(p)}</span><span><b>${p.name}</b><small>${p.desc}</small></span></button>`).join('')}</div>
    </div>
    <div class="card"><div class="sub-h">Colors</div>
      ${COLORS.map(([key, label, desc]) => `<div class="crow"><div class="row-text"><label class="row-title" for="lc-${key}">${label}</label>${desc ? `<span class="row-desc">${desc}</span>` : ''}</div>
        <span class="colorpick"><input type="color" id="lc-${key}" data-color="${key}" value="${a[key].toLowerCase()}"><input class="input" id="lh-${key}" data-hex="${key}" value="${a[key]}" maxlength="7" spellcheck="false" autocomplete="off" aria-label="${label}, hex code"></span></div>`).join('')}
      <p class="fhint bad look-warn" id="look-warn" hidden></p>
    </div>
    <div class="card"><div class="sub-h">Items</div>
      ${opacityRow(a)}
      ${sizeRows(a)}
      <div class="row"><div class="row-text"><label class="row-title" for="lk-shadow">Shadow</label><span class="row-desc">A soft shadow around the items, so they stand out on light windows.</span></div><input type="checkbox" class="switch" id="lk-shadow" ${a.shadow ? 'checked' : ''}></div>
      <div class="row"><div class="row-text"><span class="row-title" id="lbl-lk-label">Labels</span><span class="row-desc">The name next to the item under the cursor.</span></div>
        <div class="seg" role="radiogroup" aria-labelledby="lbl-lk-label"><input type="radio" name="lk-label" id="lkl-light" value="light" ${a.label === 'light' ? 'checked' : ''}><label for="lkl-light">Light</label><input type="radio" name="lk-label" id="lkl-dark" value="dark" ${a.label === 'dark' ? 'checked' : ''}><label for="lkl-dark">Dark</label></div></div>
    </div>
    <div class="card"><div class="row"><span class="row-ic">${drawIcon('image')}</span><div class="row-text"><span class="row-title">Icons</span><span class="row-desc">Each item’s icon is chosen in Ring: a built-in icon, the first letter of its name, or an SVG or PNG file of your own.</span></div><button type="button" class="btn" data-goto-ring>Open Ring</button></div></div>
    <button type="button" class="btn start" id="lk-reset" ${isDefault(a) ? 'disabled' : ''}>${drawIcon('rotate-ccw')}Restore the default appearance</button>`;
  updateWarnings();
  stage.rebuild();
}

/** The page is on screen: the ring opens in the preview. */
export function shown() {
  stage.show();
}

const swatch = p => [0, 1, 2, 3].map(i => `<i style="background:${i === 1 ? p.highlight_color : rgba(p.item_color, p.item_opacity)}"></i>`).join('');
const rgba = (hex, alpha) => `rgba(${channels(hex).map(v => Math.round(v * 255)).join(',')},${alpha})`;
const isDefault = a => JSON.stringify({ ...a, ring_radius_px: radius(a) }) === JSON.stringify(store.defaults.appearance);

function opacityRow(a) {
  const [min, max] = store.limits[OPACITY];
  return sliderRow({
    path: OPACITY, label: 'Opacity', min, max, step: 0.01, value: a.item_opacity, defaultValue: get(store.defaults, OPACITY),
    format: v => `${Math.round(v * 100)}%`, note: 'Lower lets the windows behind show through the items. The highlight stays solid.',
  });
}

function sizeRows(a) {
  const [min, max] = store.limits[SIZE], [rmin, rmax] = radiusRange(a.item_size_px), auto = automatic(a);
  return sliderRow({
    path: SIZE, label: 'Item size', min, max, step: 1, value: a.item_size_px, defaultValue: get(store.defaults, SIZE),
    format: v => `${v} px`, note: 'At 100% scaling; Thumbring grows it with the monitor scale. The center button follows it.',
  }) + sliderRow({
    path: RADIUS, label: 'Distance from the center', min: rmin, max: rmax, step: 0.1, value: radius(a), defaultValue: defaultRadius(a.item_size_px),
    format: v => `${Number(v).toFixed(1)} px`, note: 'From the center of the ring to the center of each item. Folders open twice as far.',
  }) + `<label class="tgl sub-tgl" for="lk-auto"><input type="checkbox" class="switch" id="lk-auto" ${auto ? 'checked' : ''}>Follow the item size (1.8 times it)</label>`;
}

function updateWarnings() {
  const el = $('#look-warn', root), w = lookWarnings(look());
  if (!el) return;
  el.hidden = !w.length;
  el.textContent = w.join(' ');
}

/** After a change that does not draw the page again: the presets, the reset button and the warnings. */
function syncLook() {
  const a = look(), preset = presetOf(a);
  for (const b of $$('#look-controls .preset', root)) b.setAttribute('aria-pressed', String(b.dataset.preset === preset));
  const reset = $('#lk-reset', root);
  if (reset) reset.disabled = isDefault(a);
  updateWarnings();
}

function setColor(key, value) {
  change(s => { s.appearance[key] = value; }, { key: `appearance.${key}`, render: false });
  const picker = $(`#lc-${key}`, root), hex = $(`#lh-${key}`, root);
  if (picker && picker.value.toUpperCase() !== value) picker.value = value.toLowerCase();
  if (hex && document.activeElement !== hex) hex.value = value;
  syncLook();
}

/** Shows a slider's new value next to it. */
function showValue(input, text, changed) {
  const out = $(`#${input.id}-out`, root);
  out.textContent = text;
  out.classList.toggle('changed', changed);
}

function onInput(e) {
  const t = e.target;
  if (t.dataset.color) setColor(t.dataset.color, t.value.toUpperCase());
  else if (t.dataset.hex) {
    const m = /^#?([0-9a-f]{6})$/i.exec(t.value.trim());
    t.toggleAttribute('aria-invalid', !m);
    if (m) setColor(t.dataset.hex, `#${m[1].toUpperCase()}`);
  } else if (t.dataset.path === OPACITY) {
    const value = Number(t.value);
    change(s => set(s, OPACITY, value), { key: OPACITY, render: false });
    showValue(t, `${Math.round(value * 100)}%`, value !== get(store.defaults, OPACITY));
    syncLook();
  } else if (t.dataset.path === SIZE) {
    const size = Number(t.value);
    change(s => {
      const a = s.appearance;
      // An automatic radius keeps following; one of its own stays in range.
      if (automatic(a)) a.ring_radius_px = null;
      else a.ring_radius_px = clamp(a.ring_radius_px, radiusRange(size));
      a.item_size_px = size;
    }, { key: SIZE, render: false });
    showValue(t, `${size} px`, size !== get(store.defaults, SIZE));
    // The distance slider gets the range of the new size, in place: drawing
    // it again would stop the drag on this one.
    const a = look(), distance = $(`[data-path="${RADIUS}"]`, root), [min, max] = radiusRange(size);
    Object.assign(distance, { min, max, value: radius(a) });
    showValue(distance, `${radius(a).toFixed(1)} px`, !automatic(a));
    $(`#${distance.id}-out`, root).title = `Default: ${defaultRadius(size).toFixed(1)} px`;
    syncLook();
  } else if (t.dataset.path === RADIUS) {
    const value = Number(t.value);
    change(s => { s.appearance.ring_radius_px = value; }, { key: RADIUS, render: false });
    showValue(t, `${value.toFixed(1)} px`, Math.abs(value - defaultRadius(look().item_size_px)) >= 0.05);
    $('#lk-auto', root).checked = automatic(look());
    syncLook();
  }
}

function onChangeEvent(e) {
  const t = e.target;
  if (t.id === 'lk-shadow') {
    const value = t.checked;
    change(s => { s.appearance.shadow = value; });
    $('#lk-shadow', root)?.focus();
  } else if (t.name === 'lk-label') {
    const value = t.value;
    change(s => { s.appearance.label = value; });
    $(`#lkl-${value}`, root)?.focus();
  } else if (t.id === 'lk-auto') {
    const on = t.checked;
    // Off keeps the distance as it is now, written in the file.
    change(s => { s.appearance.ring_radius_px = on ? null : radius(s.appearance); });
    $('#lk-auto', root)?.focus();
  } else if (t.dataset.hex) {
    // A half-typed code goes back to the color in use.
    t.value = look()[t.dataset.hex];
    t.removeAttribute('aria-invalid');
  }
}

function onClick(e) {
  const p = e.target.closest('[data-preset]');
  if (p) {
    const k = p.dataset.preset;
    change(s => { Object.assign(s.appearance, presetValues(k)); });
    $(`[data-preset="${k}"]`, root)?.focus();
  } else if (e.target.closest('#lk-reset')) {
    change(s => { s.appearance = structuredClone(store.defaults.appearance); });
    toast('The default appearance is back', { action: 'Undo', run: undo });
  } else if (e.target.closest('[data-goto-ring]')) goto('ring');
}

async function showOnScreen() {
  const b = $('#look-screen', root);
  b.disabled = true;
  try {
    await invoke('show_on_screen', { settings: store.settings });
    toast('Thumbring shows your ring on the screen for a few seconds, as it is here, saved or not');
  } catch (e) {
    toast(`The ring can’t be shown: ${e}`);
  } finally {
    setTimeout(() => { b.disabled = false; }, 3000);
  }
}
