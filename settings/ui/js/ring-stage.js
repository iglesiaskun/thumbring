// A live preview of the ring, drawn as the engine draws it.
//
// This is a port, to WebGL and JavaScript, of the engine's ring: the state
// machine (core/src/ring_machine.rs), the geometry (core/src/layout.rs), the
// animation curves and the blob (core/src/anim.rs) and the SDF shader
// (engine/src/ring/ring.hlsl). A change there must be made here too, or the
// preview stops matching the ring.
//
// Two modes:
// - "edit": the ring stays open for editing. Hover highlights, a click
//   selects, empty slots show as dashed circles and the item under the
//   cursor gets a × to remove it.
// - "demo": the real interaction, with the state machine, for the
//   animation settings.

import { icon, isTyping } from './ui.js';
import { itemGlyph } from './ring-icons.js';
import { commandShort } from './commands.js';

export const STAGE_ICONS = ['plus', 'x'];

/* ---------- Helpers ---------- */

const clamp01 = x => Math.max(0, Math.min(1, x));
const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
const lerp = (a, b, t) => a + (b - a) * t;
const easeOut = t => 1 - Math.pow(1 - t, 3);
const easeIn = t => t * t * t;
const easeInOut = t => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const easeOutBack = (t, s) => 1 + (s + 1) * Math.pow(t - 1, 3) + s * Math.pow(t - 1, 2);
const smooth = (e0, e1, x) => { const t = clamp01((x - e0) / (e1 - e0)); return t * t * (3 - 2 * t); };
const prog = (t, d) => (d > 0 ? clamp01(t / d) : 1);
const wait = ms => new Promise(r => setTimeout(r, ms));
const rgb = hex => {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex ?? '');
  const n = m ? parseInt(m[1], 16) : 0xffffff;
  return [(n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255];
};

/** The ring radius when the settings set none: 1.8 items, to a tenth of a pixel. */
export const defaultRadius = itemSize => Math.round(itemSize * 18) / 10;

/** The default appearance (core/src/config.rs): white items, a near-black highlight. */
export const CLASSIC = {
  item_color: '#FFFFFF', item_opacity: 0.96, icon_color: '#2A2B31', highlight_color: '#121214',
  highlight_icon_color: '#FFFFFF', cancel_color: '#E64054', item_size_px: 52, ring_radius_px: 93.6,
  shadow: true, label: 'light',
};

/* ---------- The state machine (core/src/ring_machine.rs) ---------- */

export const RingMachine = (() => {
  const initial = () => ({ phase: 'idle', hover: null, folder: null, pending: null, triggerHeld: false });
  const isOpen = s => s.phase === 'open-held' || s.phase === 'open-click';
  const isRingItem = t => !!t && (t.kind === 'action' || t.kind === 'folder');
  const isRunnable = t => !!t && (t.kind === 'action' || t.kind === 'child');
  function close(s, reason, pre = []) {
    const fx = [...pre];
    if (s.pending) fx.push({ type: 'cancelIntent' });
    fx.push({ type: 'closeRing', reason });
    return { s: { ...initial(), triggerHeld: s.triggerHeld }, fx };
  }
  function reduce(s, ev, cfg) {
    switch (ev.type) {
      case 'TRIGGER_DOWN': {
        const t = { ...s, triggerHeld: true };
        if (s.phase === 'idle') {
          if (cfg.openWith === 'tap') return { s: { ...t, phase: 'open-click' }, fx: [{ type: 'openRing' }] };
          return { s: { ...t, phase: 'armed' }, fx: [{ type: 'startHold' }] };
        }
        if (s.phase === 'open-click') return close(t, 'trigger');
        return { s: t, fx: [] };
      }
      case 'HOLD_ELAPSED':
        if (s.phase !== 'armed') return { s, fx: [] };
        return { s: { ...s, phase: 'open-held' }, fx: [{ type: 'openRing' }] };
      case 'TRIGGER_UP': {
        const t = { ...s, triggerHeld: false };
        if (s.phase === 'armed') return { s: { ...t, phase: 'idle' }, fx: [{ type: 'cancelHold' }, { type: 'passNative' }] };
        if (s.phase === 'open-held') {
          const h = s.hover;
          // Only quick mode acts on the release: over an action it runs it,
          // over the center it closes. Otherwise the ring stays open.
          if (h && h.kind === 'center' && cfg.executeOnRelease) return close(t, 'center');
          if (isRunnable(h) && cfg.executeOnRelease) return close(t, 'execute', [{ type: 'execute', target: h }]);
          return { s: { ...t, phase: 'open-click' }, fx: [{ type: 'stayOpen' }] };
        }
        return { s: t, fx: s.triggerHeld ? [{ type: 'swallowUp' }] : [] };
      }
      case 'POINTER': {
        if (!isOpen(s)) return { s, fx: [] };
        const t = ev.target, fx = [], n = { ...s };
        if (n.pending && !(isRingItem(t) && t.slot === n.pending.slot)) { n.pending = null; fx.push({ type: 'cancelIntent' }); }
        if (t.kind === 'child') n.hover = t;
        else if (t.kind === 'center') {
          n.hover = t;
          if (n.folder !== null) { fx.push({ type: 'closeFolder', slot: n.folder }); n.folder = null; }
        } else if (isRingItem(t)) {
          if (n.folder !== null && t.slot !== n.folder) {
            // With a folder open, another item takes the highlight only if the cursor rests on it.
            if (!n.pending) { n.pending = t; fx.push({ type: 'startIntent', target: t }); }
            n.hover = null;
          } else {
            n.hover = t;
            if (t.kind === 'folder' && n.folder !== t.slot) { n.folder = t.slot; fx.push({ type: 'openFolder', slot: t.slot }); }
          }
        } else n.hover = null;
        return { s: n, fx };
      }
      case 'INTENT_ELAPSED': {
        if (!isOpen(s) || !s.pending || s.pending.slot !== ev.slot) return { s, fx: [] };
        const p = s.pending, fx = [];
        if (s.folder !== null) fx.push({ type: 'closeFolder', slot: s.folder });
        const n = { ...s, pending: null, hover: p, folder: null };
        if (p.kind === 'folder') { n.folder = p.slot; fx.push({ type: 'openFolder', slot: p.slot }); }
        return { s: n, fx };
      }
      case 'CLICK': {
        if (!isOpen(s)) return { s, fx: [] };
        const t = ev.target;
        if (isRunnable(t)) return close(s, 'execute', [{ type: 'execute', target: t }]);
        if (t.kind === 'folder') {
          if (s.folder === t.slot) return { s, fx: [] };
          const fx = [];
          if (s.pending) fx.push({ type: 'cancelIntent' });
          if (s.folder !== null) fx.push({ type: 'closeFolder', slot: s.folder });
          fx.push({ type: 'openFolder', slot: t.slot });
          return { s: { ...s, folder: t.slot, hover: t, pending: null }, fx };
        }
        if (t.kind === 'center') return close(s, 'center');
        if (t.kind === 'outside') return close(s, 'outside', [{ type: 'swallowClick' }]);
        return { s, fx: [{ type: 'ignoreClick' }] };
      }
      case 'ESC':
      case 'FOCUS_LOST':
        if (isOpen(s)) return close(s, 'esc');
        if (s.phase === 'armed') return { s: { ...s, phase: 'idle' }, fx: [{ type: 'cancelHold' }] };
        return { s, fx: [] };
      default:
        return { s, fx: [] };
    }
  }
  return { initial, reduce, isOpen };
})();

/* ---------- The shader (engine/src/ring/ring.hlsl) ---------- */

const VS = 'attribute vec2 a; void main(){ gl_Position = vec4(a, 0.0, 1.0); }';
const FS = `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform vec2 uRes; uniform float uDpr;
uniform vec4 uW[32]; uniform int uWN;
uniform vec4 uD[24]; uniform int uDN;
uniform vec4 uC;
uniform vec4 uB; uniform vec4 uBE;
uniform vec4 uH; uniform vec4 uHE;
uniform float uKw; uniform float uKd; uniform float uA;
uniform vec3 uDC;
uniform vec3 uIC; uniform float uIA;
uniform vec3 uRC; uniform float uSh;
float smin(float a, float b, float k) {
  float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
  return mix(b, a, h) - k * h * (1.0 - h);
}
float sdEll(vec2 p, vec4 c, vec4 e) {
  vec2 d = p - c.xy;
  vec2 q = vec2(dot(d, e.xy), dot(d, vec2(-e.y, e.x))) / e.zw;
  return (length(q) - c.z) * min(e.z, e.w);
}
void main() {
  vec2 p = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y) / uDpr;
  float dw = 10000.0;
  for (int i = 0; i < 32; i++) { if (i >= uWN) break; vec4 c = uW[i]; dw = smin(dw, length(p - c.xy) - c.z, uKw); }
  if (uH.w > 0.5) dw = smin(dw, sdEll(p, uH, uHE), uKw);
  float dd = 10000.0;
  for (int i = 0; i < 24; i++) { if (i >= uDN) break; vec4 c = uD[i]; dd = smin(dd, length(p - c.xy) - c.z, uKd); }
  if (uB.w > 0.5) dd = smin(dd, sdEll(p, uB, uBE), uKd);
  float aw = clamp(0.5 - dw * uDpr, 0.0, 1.0);
  float shadow = (1.0 - smoothstep(0.0, 14.0, dw)) * uSh * (1.0 - aw);
  float ad = clamp(0.5 - dd * uDpr, 0.0, 1.0);
  float dc = length(p - uC.xy) - uC.z;
  float ac = clamp(0.5 - dc * uDpr, 0.0, 1.0) * mix(uIA * 0.78, 0.95, uC.w);
  vec3 cc = mix(uIC, uRC, uC.w);
  vec4 col = vec4(0.0, 0.0, 0.0, shadow);
  float wa = aw * uIA;
  col = vec4(uIC * wa, wa) + col * (1.0 - wa);
  col = vec4(cc * ac, ac) + col * (1.0 - ac);
  col = vec4(uDC * ad, ad) + col * (1.0 - ad);
  gl_FragColor = col * uA;
}`;
const MAXW = 32, MAXD = 24, NECK = 6;

/**
 * A preview in `root`, which holds a canvas and the layers of the markup in
 * pages/ring.js. Options:
 * - mode: 'edit' or 'demo'; getSlots(), getAnim(), getLook()
 * - slowmo(), reduced(): slower or shorter animations
 * - edit: isDragging(), onPointerDown(target, event), onRemove(target)
 * - demo: getBehavior(), active(), blocked(), triggerButton(), triggerName(), autoReopen
 * - onFit(ratio): the ring is drawn at this part of its real size (1 when it
 *   fits the preview)
 */
export function createRingStage(root, o) {
  const edit = o.mode === 'edit';
  const canvas = root.querySelector('canvas'), iconLayer = root.querySelector('.icons'), overlay = root.querySelector('.overlay');
  const labelEl = root.querySelector('.label'), toastEl = root.querySelector('.stoast'), fake = root.querySelector('.fake-cursor');
  const G = { D: 52, r: 26, R: 93.6, R2: 187.2, rc: 18.2, zone: 42, spacing: Math.PI / 8 };
  const wBuf = new Float32Array(MAXW * 4), dBuf = new Float32Array(MAXD * 4);
  let SL = o.getSlots(), dpr = 1, GL = null, ctx2d = null;
  const A = () => o.getAnim();
  // Without a radius of its own, the ring radius follows the item size.
  const LK = () => {
    const L = o.getLook?.() ?? CLASSIC;
    return L.ring_radius_px == null ? { ...L, ring_radius_px: defaultRadius(L.item_size_px) } : L;
  };
  const beh = () => o.getBehavior?.() ?? {};
  const dur = ms => (ms * (o.slowmo?.() ? 5 : 1) * (o.reduced?.() ? 0.35 : 1)) / A().speed;

  /* ---------- appearance: icon and label colors, size ---------- */
  let lookKey = '';
  function applyLook() {
    const L = LK(), key = [L.icon_color, L.highlight_icon_color, L.label, L.item_size_px, L.ring_radius_px, root.clientWidth, root.clientHeight].join('|');
    if (key === lookKey) return;
    lookKey = key;
    root.style.setProperty('--item-ink', L.icon_color);
    root.style.setProperty('--item-ink-on', L.highlight_icon_color);
    const dark = L.label === 'dark';
    root.style.setProperty('--pill-bg', dark ? 'rgba(24, 25, 30, 0.92)' : 'rgba(255, 255, 255, 0.94)');
    root.style.setProperty('--pill-ink', dark ? '#F1F2F5' : '#24252B');
    root.style.setProperty('--pill-sub', dark ? '#A6AAB6' : '#6A6E79');
    computeGeometry(root.clientWidth, root.clientHeight);
    if (V.label.target && itemOf(V.label.target)) setLabelContent(V.label.target);
  }

  /* ---------- geometry (core/src/layout.rs) ---------- */
  const slotAngle = i => -Math.PI / 2 + i * Math.PI / 4;
  const heroIndex = n => Math.floor(n / 2);
  const slotRel = i => ({ x: Math.cos(slotAngle(i)) * G.R, y: Math.sin(slotAngle(i)) * G.R });
  const nItems = s => (SL[s]?.type === 'folder' ? SL[s].items.length : 0);
  function childRel(slot, j) { const n = nItems(slot), a = slotAngle(slot) + (j - (n - 1) / 2) * G.spacing; return { x: Math.cos(a) * G.R2, y: Math.sin(a) * G.R2 }; }
  function ghostRel(slot) { const n = nItems(slot), a = n === 0 ? slotAngle(slot) : slotAngle(slot) + (n - (n - 1) / 2) * G.spacing; return { x: Math.cos(a) * G.R2, y: Math.sin(a) * G.R2 }; }
  function computeGeometry(w, h) {
    if (!w || !h) return;
    // The real size from Appearance, unless the preview is too small for the
    // open folder arc: then the ring shrinks as a whole, as on a small monitor
    // (Geometry::for_monitor).
    const L = LK(), size = L.item_size_px, perItem = L.ring_radius_px / size;
    const fits = (Math.min(w, h) - 24) / (4 * perItem + 1.2);
    const D = Math.max(Math.min(size, fits), 20);
    G.D = D; G.r = D / 2; G.R = perItem * D; G.R2 = 2 * G.R; G.rc = 0.35 * D; G.zone = 0.45 * G.R;
    // Children at least 22.5° apart, more if they would touch (Geometry::with_radius).
    const closest = (1 + 0.14) * D / (2 * G.R2);
    G.spacing = clamp(2 * Math.asin(Math.min(closest, 1)), Math.PI / 8, 2 * Math.PI / 9);
    root.style.setProperty('--d', D + 'px');
    o.onFit?.(D / size);
  }

  /* ---------- state ---------- */
  let state = RingMachine.initial(), pointer = { x: -9999, y: -9999 }, lastTarget = { kind: 'none' };
  let holdTimer = 0, intentTimer = 0, triggerIsDown = false, virtualMode = false, lastTriggerAt = -1e9, reopenTimer = 0, quiet = false, demoBusy = false;
  const V = { ring: null, folders: [], dark: new Map(), center: { on: false, t: 0, v0: 0 }, label: { key: null, target: null, last: null, t: 0, from: 0, w: 0, h: 0 }, pressedKey: null };
  const E = { hover: null, sel: null, drop: null, dropOk: true, peek: null, badgeOff: false };
  const keyOf = t => (t.kind === 'child' ? `c${t.slot}-${t.index}` : 's' + t.slot);
  const itemOf = t => (!t ? null : t.kind === 'child' ? SL[t.slot]?.items?.[t.index] ?? null : SL[t.slot]);
  const sameT = (a, b) => !!a && !!b && a.kind === b.kind && a.slot === b.slot && a.index === b.index;
  const eqT = (a, b) => (!a && !b) || sameT(a, b);

  /* ---------- hit-test ---------- */
  const folderEntry = slot => V.folders.find(f => f.slot === slot && !f.closing);
  const openFolderSlot = () => { const f = [...V.folders].reverse().find(x => !x.closing); return f ? f.slot : null; };
  function hitTest(p) {
    const g = V.ring;
    if (!g || g.closing) return { kind: 'none' };
    const dx = p.x - g.cx, dy = p.y - g.cy, d = Math.hypot(dx, dy), hitR = G.r * 1.12;
    const fs = edit ? openFolderSlot() : state.folder;
    if (fs !== null && fs !== undefined && SL[fs]?.type === 'folder') {
      const f = folderEntry(fs);
      if (f) {
        const a = folderAnim(f, performance.now());
        for (let j = 0; j < a.n; j++) {
          if (a.children[j] < 0.6) continue;
          const c = childRel(fs, j);
          if (Math.hypot(dx - c.x, dy - c.y) <= hitR) return { kind: 'child', slot: fs, index: j };
        }
        if (edit && a.n < 9 && (a.n === 0 || Math.min(...a.children) >= 0.6)) {
          const c = ghostRel(fs);
          if (Math.hypot(dx - c.x, dy - c.y) <= hitR) return { kind: 'ghost', slot: fs };
        }
      }
    }
    if (d <= G.zone) return { kind: 'center' };
    for (let i = 0; i < 8; i++) {
      const s = SL[i];
      if (!s || (s.type === 'empty' && !edit)) continue;
      const c = slotRel(i);
      if (Math.hypot(dx - c.x, dy - c.y) <= hitR) return { kind: s.type === 'folder' ? 'folder' : s.type === 'empty' ? 'empty' : 'action', slot: i };
    }
    if (d <= G.R2 + G.r + 6) return { kind: 'gap' };
    return { kind: 'outside' };
  }

  /* ---------- state machine effects (demo) ---------- */
  function dispatch(ev) {
    const { s, fx } = RingMachine.reduce(state, ev, beh());
    state = s;
    const now = performance.now();
    for (const e of fx) applyFx(e, now);
    if (!V.ring || !V.ring.closing) syncVisuals(now);
    kick();
  }
  function applyFx(e, now) {
    switch (e.type) {
      case 'openRing': openRingVisual(now); break;
      case 'closeRing': closeRingVisual(e.reason, now); break;
      case 'startHold': clearTimeout(holdTimer); holdTimer = setTimeout(() => dispatch({ type: 'HOLD_ELAPSED' }), beh().holdMs); break;
      case 'cancelHold': clearTimeout(holdTimer); break;
      case 'startIntent': clearTimeout(intentTimer); intentTimer = setTimeout(() => dispatch({ type: 'INTENT_ELAPSED', slot: e.target.slot }), beh().intentMs); break;
      case 'cancelIntent': clearTimeout(intentTimer); break;
      case 'openFolder':
        V.folders.forEach(f => { if (!f.closing) { f.closing = true; f.tc = now; } });
        V.folders.push({ slot: e.slot, t0: now, closing: false, tc: 0 });
        break;
      case 'closeFolder':
        V.folders.forEach(f => { if (f.slot === e.slot && !f.closing) { f.closing = true; f.tc = now; } });
        break;
      case 'execute': {
        const it = itemOf(e.target);
        V.pressedKey = keyOf(e.target);
        if (it) toast('Ran: ' + it.name, commandShort(it.command));
        break;
      }
      case 'passNative': toast(`Quick tap: ${o.triggerName()} works as usual`, 'the ring did not open'); break;
      case 'stayOpen': toast('Released: the ring stays open', 'pick with a click'); break;
      default: break;
    }
  }
  function openRingVisual(now) {
    const w = root.clientWidth, h = root.clientHeight, m = G.R2 + G.r + 10;
    const cx = w > 2 * m ? clamp(pointer.x, m, w - m) : w / 2;
    const cy = h > 2 * m ? clamp(pointer.y, m, h - m) : h / 2;
    startRing(cx, cy, now, true);
  }
  function startRing(cx, cy, now, animate) {
    clearTimeout(reopenTimer);
    V.ring = { cx, cy, t0: animate ? now : now - 10000, closing: false, tc: 0 };
    V.folders = []; V.dark.clear(); V.pressedKey = null;
    V.center = { on: false, t: now, v0: 0 };
    V.label = { key: null, target: null, last: null, t: now, from: 0, w: 0, h: 0 };
    lastTarget = { kind: 'none' };
  }
  function closeRingVisual(reason, now) {
    if (!V.ring) return;
    V.ring.closing = true; V.ring.tc = now;
    clearTimeout(intentTimer);
    if (V.pressedKey) setDark(V.pressedKey, true, now);
    lastTarget = { kind: 'none' };
    const msg = { center: ['Closed without doing anything', 'center'], outside: ['Closed: click outside the ring', 'the click is dropped'], esc: ['Closed', 'Esc or focus change'], trigger: ['Closed', 'button pressed again'] }[reason];
    if (msg && !quiet) toast(msg[0], msg[1]);
  }

  /* ---------- highlight, center, label ---------- */
  function darkVal(a, now) { const p = prog(now - a.t, dur(70)); return a.on ? lerp(Math.max(a.v0, 0.6), 1, easeOut(p)) : a.v0 * (1 - easeIn(p)); }
  function setDark(k, on, now) { const a = V.dark.get(k); if (a && a.on === on) return; V.dark.set(k, { on, t: now, v0: a ? darkVal(a, now) : 0 }); }
  function darkOf(k, now) {
    const a = V.dark.get(k);
    if (!a) return { v: 0, active: false };
    const v = darkVal(a, now), active = now - a.t < dur(70);
    if (!a.on && !active) V.dark.delete(k);
    return { v, active };
  }
  function centerVal(now) { const c = V.center, p = prog(now - c.t, dur(80)); return c.on ? lerp(c.v0, 1, p) : lerp(c.v0, 0, p); }
  function setLabel(now, h, show) {
    const lk = show ? keyOf(h) : null;
    if (lk === V.label.key) return;
    const from = parseFloat(labelEl.style.opacity) || 0;
    V.label = { key: lk, target: show ? h : null, last: show ? h : V.label.target || V.label.last, t: now, from, w: V.label.w, h: V.label.h };
    if (show) setLabelContent(h);
  }
  function syncVisuals(now) {
    if (edit) return editSync(now);
    const want = new Set(), h = state.hover;
    if (h && (h.kind === 'action' || h.kind === 'folder')) want.add('s' + h.slot);
    if (state.folder !== null) want.add('s' + state.folder);
    if (h && h.kind === 'child') want.add(keyOf(h));
    for (const [k, a] of V.dark) if (a.on && !want.has(k)) setDark(k, false, now);
    for (const k of want) setDark(k, true, now);
    const cOn = !!h && h.kind === 'center';
    if (cOn !== V.center.on) V.center = { on: cOn, t: now, v0: centerVal(now) };
    setLabel(now, h, !!h && (h.kind === 'action' || h.kind === 'child'));
  }
  function editSync(now) {
    const want = new Set(), h = E.hover, of = openFolderSlot();
    if (h && (h.kind === 'action' || h.kind === 'folder')) want.add('s' + h.slot);
    if (h && h.kind === 'child') want.add(keyOf(h));
    if (of !== null) want.add('s' + of);
    for (const [k, a] of V.dark) if (a.on && !want.has(k)) setDark(k, false, now);
    for (const k of want) setDark(k, true, now);
    const cOn = !!h && h.kind === 'center';
    if (cOn !== V.center.on) V.center = { on: cOn, t: now, v0: centerVal(now) };
    setLabel(now, h, !!h && (h.kind === 'action' || h.kind === 'child' || (h.kind === 'folder' && of !== h.slot)));
  }
  function setLabelContent(h) {
    const it = itemOf(h);
    if (!it) return;
    labelEl.querySelector('.nm').textContent = it.name || 'Untitled';
    const sub = commandShort(it.command);
    labelEl.querySelector('.k').textContent = h.kind === 'folder'
      ? `folder · ${it.items.length} ${it.items.length === 1 ? 'action' : 'actions'}`
      : sub.toLowerCase() === (it.name ?? '').trim().toLowerCase() ? '' : sub;
    V.label.w = labelEl.offsetWidth; V.label.h = labelEl.offsetHeight;
  }
  function drawLabel(now, ra, P) {
    const L = V.label;
    let op = L.key ? prog(now - L.t - dur(60), dur(50)) : L.from * (1 - prog(now - L.t, dur(100)));
    const busy = L.key ? op < 1 : op > 0;
    op *= ra.alpha;
    const tg = L.key ? L.target : L.last;
    if (!tg || !itemOf(tg) || op <= 0.01) { labelEl.style.opacity = '0'; return busy; }
    const p = P(tg.kind === 'child' ? childRel(tg.slot, tg.index) : slotRel(tg.slot));
    const ang = Math.atan2(p.y - V.ring.cy, p.x - V.ring.cx), ca = Math.cos(ang), sa = Math.sin(ang);
    const w = L.w, h = L.h, gap = 10, rr = G.r * ra.size;
    let x, y;
    if (Math.abs(ca) > 0.38) { x = ca > 0 ? p.x + rr + gap : p.x - rr - gap - w; y = p.y - h / 2; }
    else { x = p.x - w / 2; y = sa > 0 ? p.y + rr + gap : p.y - rr - gap - h; }
    x = clamp(x, 6, root.clientWidth - w - 6); y = clamp(y, 6, root.clientHeight - h - 6);
    labelEl.style.transform = `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0)`;
    labelEl.style.opacity = op.toFixed(3);
    return busy;
  }

  /* ---------- animations (core/src/anim.rs) ---------- */
  function ringAnim(now) {
    const g = V.ring;
    const po = easeOut(prog(now - g.t0, dur(A().ring_open_ms)));
    let spread = lerp(0.15, 1, po), size = lerp(0.4, 1, po), alpha = clamp01(po * 1.6), pc = 0;
    if (g.closing) {
      pc = prog(now - g.tc, dur(A().ring_close_ms));
      const e = easeInOut(pc);
      spread *= lerp(1, 0.15, e); size *= lerp(1, 0.35, e); alpha *= 1 - Math.pow(pc, 0.75);
    }
    return { spread, size, alpha, pc, active: po < 1 || g.closing };
  }
  function travelElastic(out, g, q) {
    const { F, L, dx, dy } = g, vis = A().viscosity, QB = A().break_at;
    const fB = 0.45 + 0.35 * clamp(vis, 0, 2) / 2;
    const neckBase = G.r * Math.min(0.7, 0.3 + 0.25 * vis);
    if (q < QB) {
      const a = q / QB, fr = fB * Math.pow(a, 1.4);
      const B = { x: F.x + dx * L * fr, y: F.y + dy * L * fr }, rb = G.r * lerp(0.3, 0.62, a);
      const sx = 1 + 0.28 * vis * a;
      out.blob = { x: B.x, y: B.y, r: rb, dx, dy, sx, sy: 1 / Math.sqrt(sx) };
      const pinch = 0.9 * a * a;
      for (let i = 1; i <= NECK; i++) { const s = i / (NECK + 1); out.neck.push({ x: lerp(F.x, B.x, s), y: lerp(F.y, B.y, s), r: lerp(neckBase, rb * 0.8, s) * (1 - pinch * Math.sin(Math.PI * s)) }); }
    } else {
      const u = (q - QB) / (1 - QB), fr = fB + (1 - fB) * easeOutBack(u, 1.6 * A().wobble);
      const B = { x: F.x + dx * L * fr, y: F.y + dy * L * fr }, rb = G.r * lerp(0.62, 1, easeOut(u));
      const sx = lerp(1 + 0.28 * vis, 1, easeOut(u));
      out.blob = { x: B.x, y: B.y, r: rb, dx, dy, sx, sy: 1 / Math.sqrt(sx) };
      const sn = easeOut(Math.min(1, u / 0.4)), Bb = { x: F.x + dx * L * fB, y: F.y + dy * L * fB };
      for (let i = 1; i <= NECK; i++) {
        const s = i / (NECK + 1);
        const r0 = lerp(neckBase, G.r * 0.62 * 0.8, s) * (1 - 0.9 * Math.sin(Math.PI * s));
        const p0 = { x: lerp(F.x, Bb.x, s), y: lerp(F.y, Bb.y, s) }, to = s < 0.5 ? F : B;
        out.neck.push({ x: lerp(p0.x, to.x, sn), y: lerp(p0.y, to.y, sn), r: r0 * (1 - sn) });
      }
    }
  }
  const viscousShape = vis => ({ thick: 0.55 + 0.375 * clamp(vis, 0, 2), fB: clamp(0.8 + 0.08 * vis, 0.8, 0.95) });
  function travelViscous(out, g, q, tb) {
    const { F, L, dx, dy } = g, vis = A().viscosity, QB = A().break_at, { thick, fB } = viscousShape(vis);
    if (q < QB) {
      const a = q / QB, fr = fB * easeInOut(a);
      const B = { x: F.x + dx * L * fr, y: F.y + dy * L * fr }, rb = G.r * lerp(0.35, 0.85, easeOut(a));
      const sx = 1 + 0.12 * vis * Math.sin(Math.PI * a);
      out.blob = { x: B.x, y: B.y, r: rb, dx, dy, sx, sy: 1 / Math.sqrt(sx) };
      const pinch = 0.22 + 0.73 * smooth(0.65, 1, a);
      for (let i = 1; i <= NECK; i++) { const s = i / (NECK + 1); out.neck.push({ x: lerp(F.x, B.x, s), y: lerp(F.y, B.y, s), r: lerp(G.r * 0.62, rb * 0.8, s) * thick * (1 - pinch * Math.sin(Math.PI * s)) }); }
    } else {
      const u = (q - QB) / (1 - QB), fr = fB + (1 - fB) * easeOutBack(u, 1.6 * A().wobble);
      const B = { x: F.x + dx * L * fr, y: F.y + dy * L * fr };
      out.blob = { x: B.x, y: B.y, r: G.r * lerp(0.85, 1, easeOut(u)), dx, dy, sx: 1, sy: 1 };
      neckRemnantsViscous(out, g, (q - QB) * tb, B);
    }
  }
  function neckRemnantsViscous(out, g, tAfter, B) {
    const sn = easeOut(prog(tAfter, dur(70)));
    if (sn >= 1) return;
    const { F, L, dx, dy } = g, { thick, fB } = viscousShape(A().viscosity);
    const Bb = { x: F.x + dx * L * fB, y: F.y + dy * L * fB };
    for (let i = 1; i <= NECK; i++) {
      const s = i / (NECK + 1);
      const r0 = lerp(G.r * 0.62, G.r * 0.85 * 0.8, s) * thick * (1 - 0.95 * Math.sin(Math.PI * s));
      const p0 = { x: lerp(F.x, Bb.x, s), y: lerp(F.y, Bb.y, s) }, to = s < 0.5 ? F : B;
      out.neck.push({ x: lerp(p0.x, to.x, sn), y: lerp(p0.y, to.y, sn), r: r0 * (1 - sn) });
    }
  }
  function folderAnim(f, now) {
    const n = nItems(f.slot), h = heroIndex(n);
    const out = { n, h, children: new Array(n).fill(0), blob: null, neck: [], hero: null, heroOnBlob: 0, active: true, finished: false };
    if (n > 0) {
      const F = slotRel(f.slot), H = childRel(f.slot, h);
      const L = Math.hypot(H.x - F.x, H.y - F.y), dx = (H.x - F.x) / L, dy = (H.y - F.y) / L;
      const g = { F, H, L, dx, dy }, W = A().wobble, viscous = A().style === 'viscous';
      const d0 = dur(35), tb = dur(clamp(A().blob_ms_per_item * L / G.D, 100, 520)), ts = dur(A().folder_items_ms), tf = dur(150), tw = dur(420);
      const t = now - f.t0;
      if (t >= d0 && t < d0 + tb) {
        const q = (t - d0) / tb;
        if (viscous) travelViscous(out, g, q, tb); else travelElastic(out, g, q);
        out.heroOnBlob = clamp01((q - 0.15) * 3);
      } else if (t >= d0 + tb) {
        if (viscous) neckRemnantsViscous(out, g, t - d0 - A().break_at * tb, H);
        const tl = t - d0 - tb;
        const w = W * 0.3 * Math.exp(-tl / Math.max(1, dur(110))) * Math.sin(2 * Math.PI * tl / Math.max(1, dur(150)));
        const shape = { dx, dy, sx: 1 - w, sy: 1 + w * 0.8 };
        const qf = prog(tl, tf);
        if (qf < 1) out.blob = { x: H.x, y: H.y, r: G.r * (1 - easeIn(qf)), ...shape };
        out.hero = shape;
        const s = easeOut(prog(tl, ts));
        for (let j = 0; j < n; j++) out.children[j] = j === h ? 1 : s;
        out.active = tl < Math.max(ts, tf, W > 0 ? tw : 0);
      }
    } else out.active = now - f.t0 < dur(A().folder_items_ms);
    if (f.closing) {
      const pc = prog(now - f.tc, dur(A().folder_close_ms)), k = 1 - easeIn(pc);
      for (let j = 0; j < n; j++) out.children[j] *= k;
      if (out.blob) out.blob.r *= k;
      out.neck.forEach(c => { c.r *= k; });
      out.heroOnBlob *= k;
      out.active = pc < 1;
      out.finished = pc >= 1;
    }
    return out;
  }

  /* ---------- WebGL ---------- */
  function initGL() {
    const gl = canvas.getContext('webgl', { premultipliedAlpha: true, alpha: true, antialias: false });
    if (!gl) return null;
    const sh = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) { console.warn(gl.getShaderInfoLog(s)); return null; } return s; };
    const vs = sh(gl.VERTEX_SHADER, VS), fs = sh(gl.FRAGMENT_SHADER, FS);
    if (!vs || !fs) return null;
    const pr = gl.createProgram(); gl.attachShader(pr, vs); gl.attachShader(pr, fs); gl.linkProgram(pr);
    if (!gl.getProgramParameter(pr, gl.LINK_STATUS)) return null;
    gl.useProgram(pr);
    const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(pr, 'a'); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const u = {};
    ['uRes', 'uDpr', 'uW', 'uWN', 'uD', 'uDN', 'uC', 'uB', 'uBE', 'uH', 'uHE', 'uKw', 'uKd', 'uA', 'uDC', 'uIC', 'uIA', 'uRC', 'uSh'].forEach(n => { u[n] = gl.getUniformLocation(pr, n); });
    gl.disable(gl.BLEND);
    return { gl, u };
  }
  function renderShapes(s) {
    if (GL) {
      const { gl, u } = GL;
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
      gl.uniform2f(u.uRes, canvas.width, canvas.height); gl.uniform1f(u.uDpr, dpr);
      gl.uniform4fv(u.uW, wBuf); gl.uniform1i(u.uWN, s.wn);
      gl.uniform4fv(u.uD, dBuf); gl.uniform1i(u.uDN, s.dn);
      gl.uniform4f(u.uC, s.cx, s.cy, s.cr, s.cv);
      const b = s.blob, hh = s.hero;
      gl.uniform4f(u.uB, b ? b.x : 0, b ? b.y : 0, b ? b.r : 0, b ? 1 : 0);
      gl.uniform4f(u.uBE, b ? b.dx : 1, b ? b.dy : 0, b ? b.sx : 1, b ? b.sy : 1);
      gl.uniform4f(u.uH, hh ? hh.x : 0, hh ? hh.y : 0, hh ? hh.r : 0, hh ? 1 : 0);
      gl.uniform4f(u.uHE, hh ? hh.dx : 1, hh ? hh.dy : 0, hh ? hh.sx : 1, hh ? hh.sy : 1);
      gl.uniform1f(u.uKw, s.kw); gl.uniform1f(u.uKd, Math.max(0.5, s.kd)); gl.uniform1f(u.uA, s.alpha);
      gl.uniform3f(u.uDC, s.dc[0], s.dc[1], s.dc[2]);
      gl.uniform3f(u.uIC, s.ic[0], s.ic[1], s.ic[2]); gl.uniform1f(u.uIA, s.ia);
      gl.uniform3f(u.uRC, s.rc[0], s.rc[1], s.rc[2]); gl.uniform1f(u.uSh, s.sh);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      return;
    }
    if (!ctx2d) return;
    // Without WebGL: plain circles, without the blob.
    ctx2d.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx2d.clearRect(0, 0, canvas.width, canvas.height);
    ctx2d.globalAlpha = s.alpha;
    const circle = (x, y, r, fill) => { ctx2d.beginPath(); ctx2d.arc(x, y, Math.max(0, r), 0, Math.PI * 2); ctx2d.fillStyle = fill; ctx2d.fill(); };
    const css = (c, a) => `rgba(${c.map(v => Math.round(v * 255)).join(',')},${a})`;
    for (let i = 0; i < s.wn; i++) circle(wBuf[i * 4], wBuf[i * 4 + 1], wBuf[i * 4 + 2], css(s.ic, s.ia));
    if (s.hero) circle(s.hero.x, s.hero.y, s.hero.r, css(s.ic, s.ia));
    circle(s.cx, s.cy, s.cr, s.cv > 0.5 ? css(s.rc, 0.95) : css(s.ic, s.ia * 0.78));
    for (let i = 0; i < s.dn; i++) circle(dBuf[i * 4], dBuf[i * 4 + 1], dBuf[i * 4 + 2], css(s.dc, 1));
    if (s.blob) circle(s.blob.x, s.blob.y, s.blob.r, css(s.dc, 1));
    ctx2d.globalAlpha = 1;
  }
  function clearShapes() {
    if (GL) { GL.gl.viewport(0, 0, canvas.width, canvas.height); GL.gl.clearColor(0, 0, 0, 0); GL.gl.clear(GL.gl.COLOR_BUFFER_BIT); }
    else if (ctx2d) { ctx2d.setTransform(1, 0, 0, 1, 0, 0); ctx2d.clearRect(0, 0, canvas.width, canvas.height); }
  }

  /* ---------- icons and the edit overlay ---------- */
  const icons = new Map();
  function buildIcons() {
    iconLayer.textContent = ''; icons.clear();
    const mk = (key, item, cls) => { const el = document.createElement('div'); el.className = 'ico' + (cls ? ' ' + cls : ''); el.innerHTML = itemGlyph(item); iconLayer.appendChild(el); icons.set(key, el); };
    SL.forEach((s, i) => { if (!s || s.type === 'empty') return; mk('s' + i, s); if (s.type === 'folder') s.items.forEach((c, j) => mk(`c${i}-${j}`, c)); });
    mk('center', { icon: 'x', name: '×' }, 'center');
  }
  function applyIcons(map) {
    icons.forEach((el, k) => {
      const st = map?.get(k);
      if (!st || st.opacity <= 0.01 || st.scale <= 0.01) { if (el.style.opacity !== '0') el.style.opacity = '0'; return; }
      el.style.transform = `translate3d(${(st.x - G.D / 2).toFixed(2)}px, ${(st.y - G.D / 2).toFixed(2)}px, 0) scale(${st.scale.toFixed(3)})`;
      el.style.opacity = st.opacity.toFixed(3);
      el.classList.toggle('on', !!st.on);
    });
  }
  const ov = {};
  if (edit) {
    ov.ghosts = Array.from({ length: 8 }, () => { const g = document.createElement('div'); g.className = 'ghost'; g.innerHTML = icon('plus'); overlay.appendChild(g); return g; });
    ov.add = document.createElement('div'); ov.add.className = 'ghost add'; ov.add.innerHTML = icon('plus'); overlay.appendChild(ov.add);
    ov.sel = document.createElement('div'); ov.sel.className = 'selring'; overlay.appendChild(ov.sel);
    ov.drop = document.createElement('div'); ov.drop.className = 'dropring'; overlay.appendChild(ov.drop);
    ov.x = document.createElement('button'); ov.x.type = 'button'; ov.x.className = 'xbadge'; ov.x.tabIndex = -1; ov.x.innerHTML = icon('x');
    root.appendChild(ov.x);
    // The × belongs to the item under the cursor: pressing it must not select or drag the item.
    ov.x.addEventListener('pointerdown', e => { e.stopPropagation(); e.preventDefault(); });
    ov.x.addEventListener('click', e => {
      e.stopPropagation();
      if (!ov.xFor) return;
      const t = ov.xFor;
      // Hidden until the mouse moves, so a double click cannot remove two items.
      E.badgeOff = true;
      ov.x.classList.remove('show');
      o.onRemove(t);
    });
  }
  const place = (el, x, y, size, op) => {
    if (op <= 0.01 || size <= 1) { el.style.opacity = '0'; return; }
    el.style.width = el.style.height = size.toFixed(1) + 'px';
    el.style.transform = `translate3d(${(x - size / 2).toFixed(1)}px, ${(y - size / 2).toFixed(1)}px, 0)`;
    el.style.opacity = op.toFixed(3);
  };
  function placeOverlay(P, ra, itemR, lead, leadFA) {
    const kids = lead && leadFA && !lead.closing ? (leadFA.n ? Math.min(...leadFA.children) : prog(performance.now() - lead.t0, dur(A().folder_items_ms))) : 0;
    const posOf = t => {
      if (!t) return null;
      if (t.kind === 'child') {
        if (!lead || lead.slot !== t.slot || !leadFA || t.index >= leadFA.n) return null;
        return { ...P(childRel(t.slot, t.index)), s: itemR * leadFA.children[t.index] };
      }
      if (t.kind === 'ghost') return lead?.slot === t.slot ? { ...P(ghostRel(t.slot)), s: itemR * kids } : null;
      if (t.slot == null) return null;
      return { ...P(slotRel(t.slot)), s: itemR };
    };
    for (let i = 0; i < 8; i++) {
      const g = ov.ghosts[i];
      if (SL[i]?.type === 'empty') {
        const p = P(slotRel(i));
        place(g, p.x, p.y, itemR * 2, ra.alpha);
        g.classList.toggle('hot', E.hover?.kind === 'empty' && E.hover.slot === i);
      } else g.style.opacity = '0';
    }
    if (lead && !lead.closing && nItems(lead.slot) < 9 && SL[lead.slot]?.type === 'folder') {
      const p = P(ghostRel(lead.slot));
      place(ov.add, p.x, p.y, itemR * 2 * kids, ra.alpha * kids);
    } else ov.add.style.opacity = '0';
    const sp = posOf(E.sel?.index != null ? { kind: 'child', slot: E.sel.slot, index: E.sel.index } : E.sel ? { kind: 'slot', slot: E.sel.slot } : null);
    if (sp) place(ov.sel, sp.x, sp.y, sp.s * 2 + 6, ra.alpha); else ov.sel.style.opacity = '0';
    const dp = posOf(E.drop);
    if (dp) { place(ov.drop, dp.x, dp.y, Math.max(dp.s, itemR * 0.6) * 2 + 10, 1); ov.drop.classList.toggle('bad', !E.dropOk); }
    else ov.drop.style.opacity = '0';
    const bt = !E.badgeOff && !o.isDragging() && ['action', 'folder', 'child'].includes(E.hover?.kind) && itemOf(E.hover) ? E.hover : null;
    const bp = posOf(bt);
    if (bp && bp.s > G.r * 0.6 && ra.alpha > 0.5) {
      const size = Math.max(18, G.D * 0.42), k = bp.s * 0.72;
      ov.x.style.width = ov.x.style.height = size.toFixed(1) + 'px';
      ov.x.style.transform = `translate3d(${(bp.x + k - size / 2).toFixed(1)}px, ${(bp.y - k - size / 2).toFixed(1)}px, 0)`;
      if (!sameT(bt, ov.xFor)) {
        ov.x.setAttribute('aria-label', `Remove ${itemOf(bt).name || 'this item'}`);
        ov.x.title = bt.kind === 'folder' ? 'Remove the folder' : 'Remove';
      }
      ov.xFor = bt;
      ov.x.classList.add('show');
    } else { ov.xFor = null; ov.x.classList.remove('show'); }
  }

  /* ---------- frame ---------- */
  let rafId = 0;
  function kick() { if (!rafId) rafId = requestAnimationFrame(frame); }
  function frame(now) { rafId = 0; if (draw(now)) kick(); }
  function draw(now) {
    applyLook();
    if (!V.ring) { clearShapes(); applyIcons(null); labelEl.style.opacity = '0'; return false; }
    const ra = ringAnim(now);
    if (V.ring.closing && ra.pc >= 1) {
      V.ring = null; V.folders = []; V.dark.clear();
      clearShapes(); applyIcons(null); labelEl.style.opacity = '0';
      if (o.autoReopen) scheduleReopen();
      return false;
    }
    if (!edit && !V.ring.closing) refreshTarget();
    if (!V.ring) return true;
    let active = ra.active;
    const cx = V.ring.cx, cy = V.ring.cy;
    const P = rel => ({ x: cx + rel.x * ra.spread, y: cy + rel.y * ra.spread });
    const itemR = G.r * ra.size, iconMap = new Map();
    let wn = 0, dn = 0;
    const pushW = (x, y, r) => { if (wn < MAXW && r > 0.2) { wBuf[wn * 4] = x; wBuf[wn * 4 + 1] = y; wBuf[wn * 4 + 2] = r; wBuf[wn * 4 + 3] = 0; wn++; } };
    const pushD = (x, y, r) => { if (dn < MAXD && r > 0.2) { dBuf[dn * 4] = x; dBuf[dn * 4 + 1] = y; dBuf[dn * 4 + 2] = r; dBuf[dn * 4 + 3] = 0; dn++; } };
    for (let i = 0; i < 8; i++) {
      if (!SL[i] || SL[i].type === 'empty') continue;
      const p = P(slotRel(i)), dv = darkOf('s' + i, now);
      if (dv.active) active = true;
      pushW(p.x, p.y, itemR);
      if (SL[i].type === 'folder') {
        // The folder's bump, pointing out; it goes away when the folder turns dark.
        const a = slotAngle(i), nd = (G.R + G.r * 0.95) * ra.spread;
        pushW(cx + Math.cos(a) * nd, cy + Math.sin(a) * nd, G.r * 0.22 * ra.size * (1 - dv.v));
      }
      if (dv.v > 0) pushD(p.x, p.y, itemR * dv.v);
      iconMap.set('s' + i, { x: p.x, y: p.y, scale: ra.size, opacity: ra.alpha, on: dv.v > 0.45 });
    }
    V.folders = V.folders.filter(f => SL[f.slot]?.type === 'folder');
    // The newest open folder gets the blob; folders that are closing get plain circles.
    const lead = [...V.folders].reverse().find(f => !f.closing) || V.folders[V.folders.length - 1];
    let blobU = null, heroU = null, leadFA = null;
    for (const f of V.folders) {
      const fa = folderAnim(f, now), isLead = f === lead;
      if (isLead) leadFA = fa;
      if (fa.active) active = true;
      for (let j = 0; j < fa.n; j++) {
        const sc = fa.children[j], p = P(childRel(f.slot, j)), k = `c${f.slot}-${j}`;
        const dv = darkOf(k, now);
        if (dv.active) active = true;
        if (sc > 0) {
          if (isLead && j === fa.h && fa.hero) heroU = { x: p.x, y: p.y, r: itemR * sc, ...fa.hero };
          else pushW(p.x, p.y, itemR * sc);
        }
        if (dv.v > 0 && sc > 0) pushD(p.x, p.y, itemR * sc * dv.v);
        let ic = { x: p.x, y: p.y, scale: ra.size * sc, opacity: ra.alpha * clamp01(sc * 1.5), on: dv.v > 0.45 };
        if (j === fa.h && sc === 0 && fa.blob && fa.heroOnBlob > 0) {
          const b = P(fa.blob);
          ic = { x: b.x, y: b.y, scale: ra.size * (fa.blob.r / G.r), opacity: ra.alpha * fa.heroOnBlob, on: true };
        }
        if (!iconMap.has(k) || sc > 0 || ic.on) iconMap.set(k, ic);
      }
      fa.neck.forEach(c => { const q = P(c); pushD(q.x, q.y, c.r * ra.size); });
      if (fa.blob) {
        const b = P(fa.blob);
        if (isLead) blobU = { x: b.x, y: b.y, r: fa.blob.r * ra.size, dx: fa.blob.dx, dy: fa.blob.dy, sx: fa.blob.sx, sy: fa.blob.sy };
        else pushD(b.x, b.y, fa.blob.r * ra.size);
      }
      if (fa.finished) f.dead = true;
    }
    V.folders = V.folders.filter(f => !f.dead);
    const cv = centerVal(now);
    if (now - V.center.t < dur(80)) active = true;
    const L = LK(), hc = rgb(L.highlight_color), ic = rgb(L.item_color);
    renderShapes({
      wn, dn, cx, cy, cr: G.rc * ra.size, cv, alpha: ra.alpha, blob: blobU, hero: heroU,
      kw: G.r * 0.22, kd: G.r * (0.22 + 0.18 * A().viscosity),
      // Closing: highlighted items fade halfway to the item color.
      dc: V.ring.closing ? hc.map((v, i) => lerp(v, ic[i], 0.5)) : hc,
      ic, ia: L.item_opacity, rc: rgb(L.cancel_color), sh: L.shadow ? 0.1 : 0,
    });
    iconMap.set('center', { x: cx, y: cy, scale: ra.size, opacity: ra.alpha, on: cv > 0.5 });
    applyIcons(iconMap);
    if (drawLabel(now, ra, P)) active = true;
    if (edit) placeOverlay(P, ra, itemR, lead, leadFA);
    return active;
  }

  /* ---------- the preview's own toast ---------- */
  let toastTimer = 0;
  function toast(text, tag) {
    if (!toastEl) return;
    toastEl.querySelector('.t').textContent = text;
    toastEl.querySelector('.tag').textContent = tag || '';
    toastEl.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toastEl.classList.remove('show'), 2000);
  }

  /* ---------- input ---------- */
  const localXY = e => { const r = root.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  const inStage = e => { const r = root.getBoundingClientRect(); return e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom; };
  function refreshTarget() {
    if (!RingMachine.isOpen(state)) return;
    const t = hitTest(pointer);
    if (!sameT(t, lastTarget)) { lastTarget = t; dispatch({ type: 'POINTER', target: t }); }
  }
  function refreshHover() {
    const t = hitTest(pointer);
    const nt = ['action', 'folder', 'child', 'center', 'empty', 'ghost'].includes(t.kind) ? t : null;
    root.style.cursor = nt && nt.kind !== 'center' ? 'pointer' : 'default';
    if (!eqT(nt, E.hover)) { E.hover = nt; editSync(performance.now()); kick(); }
  }
  function setPointer(p) {
    pointer = p;
    if (virtualMode && fake) fake.style.transform = `translate(${(p.x - 2).toFixed(1)}px, ${(p.y - 2).toFixed(1)}px)`;
    refreshTarget();
  }
  const enterVirtual = () => { virtualMode = true; if (fake) { fake.style.display = 'block'; fake.style.transform = `translate(${pointer.x - 2}px, ${pointer.y - 2}px)`; } };
  const exitVirtual = () => { virtualMode = false; if (fake) fake.style.display = 'none'; };
  function triggerDown() { if (triggerIsDown) return; triggerIsDown = true; lastTriggerAt = performance.now(); clearTimeout(reopenTimer); dispatch({ type: 'TRIGGER_DOWN' }); }
  function triggerUp() { if (!triggerIsDown) return; triggerIsDown = false; lastTriggerAt = performance.now(); dispatch({ type: 'TRIGGER_UP' }); }
  const clickAtPointer = () => dispatch({ type: 'CLICK', target: hitTest(pointer) });

  if (edit) {
    root.addEventListener('pointermove', e => {
      // Over the ×, the item under it keeps the highlight (and the ×).
      if (ov.x.contains(e.target)) return;
      pointer = localXY(e);
      if (o.isDragging()) return;
      if (E.badgeOff) { E.badgeOff = false; kick(); }
      refreshHover();
    });
    root.addEventListener('pointerleave', () => { pointer = { x: -9999, y: -9999 }; if (E.hover) { E.hover = null; editSync(performance.now()); kick(); } });
    root.addEventListener('pointerdown', e => { if (e.button !== 0) return; pointer = localXY(e); o.onPointerDown(hitTest(pointer), e); });
  } else {
    root.addEventListener('pointermove', e => { if (e.pointerType === 'mouse' && virtualMode) exitVirtual(); if (!demoBusy) setPointer(localXY(e)); });
    window.addEventListener('mousedown', e => {
      if (!o.active() || !inStage(e) || demoBusy) return;
      if (e.button === o.triggerButton()) { e.preventDefault(); if (virtualMode) exitVirtual(); setPointer(localXY(e)); triggerDown(); return; }
      if (e.button === 0 && RingMachine.isOpen(state)) { e.preventDefault(); setPointer(localXY(e)); clickAtPointer(); }
      else if ((e.button === 2 || e.button === 1) && RingMachine.isOpen(state)) { e.preventDefault(); dispatch({ type: 'CLICK', target: { kind: 'outside' } }); }
    });
    window.addEventListener('mouseup', e => { if (e.button === o.triggerButton() && triggerIsDown) { e.preventDefault(); triggerUp(); } });
    root.addEventListener('contextmenu', e => e.preventDefault());
    root.addEventListener('pointerdown', e => {
      if (e.pointerType === 'mouse' || demoBusy) return;
      e.preventDefault(); setPointer(localXY(e));
      if (state.phase === 'idle') triggerDown(); else if (state.phase === 'open-click') clickAtPointer();
    });
    root.addEventListener('pointerup', e => { if (e.pointerType !== 'mouse' && triggerIsDown) { setPointer(localXY(e)); triggerUp(); } });
    window.addEventListener('keydown', e => {
      if (!o.active() || demoBusy || o.blocked?.()) return;
      if (e.key === 'Escape' && (RingMachine.isOpen(state) || state.phase === 'armed')) { dispatch({ type: 'ESC' }); return; }
      if (e.code === 'Space' && !isTyping() && !document.activeElement?.matches('button, input, [role="tab"]')) { e.preventDefault(); if (!e.repeat) triggerDown(); }
    });
    window.addEventListener('keyup', e => { if (e.code === 'Space' && triggerIsDown) { e.preventDefault(); triggerUp(); } });
    window.addEventListener('blur', () => { if (!o.active()) return; if (RingMachine.isOpen(state) || state.phase === 'armed') dispatch({ type: 'FOCUS_LOST' }); if (triggerIsDown) triggerUp(); });
  }

  /* ---------- demo helpers ---------- */
  function openCenter(animate = true) {
    const w = root.clientWidth, h = root.clientHeight;
    if (!w || !h) return;
    state = { ...RingMachine.initial(), phase: 'open-click' };
    startRing(w / 2, h / 2, performance.now(), animate);
    kick();
  }
  function scheduleReopen() {
    clearTimeout(reopenTimer);
    reopenTimer = setTimeout(() => {
      if (V.ring || state.phase !== 'idle' || triggerIsDown || demoBusy || !o.active()) return;
      if (performance.now() - lastTriggerAt < 4000) return;
      pointer = { x: -9999, y: -9999 };
      openCenter(true);
    }, 1100);
  }
  function closeQuiet() { quiet = true; dispatch({ type: 'ESC' }); quiet = false; }
  const firstFolder = () => SL.findIndex(s => s?.type === 'folder' && s.items.length);
  const absSlot = i => { const r = slotRel(i); return { x: V.ring.cx + r.x, y: V.ring.cy + r.y }; };
  const absChild = (s, j) => { const r = childRel(s, j); return { x: V.ring.cx + r.x, y: V.ring.cy + r.y }; };
  function glideTo(p, ms = 320) {
    enterVirtual();
    const from = { ...pointer }, t0 = performance.now();
    return new Promise(res => {
      const step = now => { const q = clamp01((now - t0) / ms), e = easeInOut(q); setPointer({ x: lerp(from.x, p.x, e), y: lerp(from.y, p.y, e) }); if (q < 1) requestAnimationFrame(step); else res(); };
      requestAnimationFrame(step);
    });
  }
  const slow = () => (o.slowmo?.() ? 5 : 1);
  /** Opens the ring, goes to the first folder and runs one of its actions. */
  async function playDemo() {
    if (demoBusy) return;
    demoBusy = true;
    try {
      clearTimeout(reopenTimer);
      if (V.ring) { closeQuiet(); await wait(dur(A().ring_close_ms) + 120); }
      const w = root.clientWidth, h = root.clientHeight;
      pointer = { x: w / 2 - 6, y: h / 2 + 8 }; enterVirtual(); setPointer(pointer);
      triggerIsDown = false; triggerDown();
      await wait(beh().holdMs + 90);
      const f = firstFolder();
      if (f < 0 || !V.ring) { triggerUp(); return; }
      await glideTo(absSlot(f), 360 * slow());
      triggerUp();
      const n = SL[f].items.length;
      await wait(dur(A().blob_ms_per_item * 2.4) + dur(A().folder_items_ms) + 260 * slow());
      if (!V.ring) return;
      await glideTo(absChild(f, Math.max(0, Math.min(n - 1, heroIndex(n) - 1))), 300 * slow());
      await wait(380 * slow());
      clickAtPointer();
      await wait(dur(A().ring_close_ms) + 300);
    } finally {
      demoBusy = false;
      lastTriggerAt = -1e9;
      if (!V.ring) scheduleReopen();
    }
  }
  /** Shows again the animation a setting changes: 'ring' (open and close) or 'folder'. */
  function replay(kind) {
    if (!o.active() || demoBusy) return;
    exitVirtual();
    if (kind === 'ring' || !V.ring || V.ring.closing) {
      if (V.ring && !V.ring.closing) {
        closeQuiet(); clearTimeout(reopenTimer);
        setTimeout(() => { pointer = { x: -9999, y: -9999 }; openCenter(true); }, dur(A().ring_close_ms) + 200);
      } else { pointer = { x: -9999, y: -9999 }; openCenter(true); }
      return;
    }
    const fs = firstFolder();
    if (fs < 0) return;
    const now = performance.now(), wasOpen = state.folder !== null;
    V.folders.forEach(f => { if (!f.closing) { f.closing = true; f.tc = now; } });
    state = { ...state, folder: null, hover: null, pending: null };
    syncVisuals(now); kick();
    setTimeout(() => {
      if (!V.ring || V.ring.closing) return;
      const t = { kind: 'folder', slot: fs };
      pointer = absSlot(fs); lastTarget = t;
      state = { ...state, folder: fs, hover: t, pending: null };
      V.folders.push({ slot: fs, t0: performance.now(), closing: false, tc: 0 });
      syncVisuals(performance.now()); kick();
    }, wasOpen ? dur(A().folder_close_ms) + 80 : 0);
  }

  /* ---------- public ---------- */
  function resize() {
    const w = root.clientWidth, h = root.clientHeight;
    if (!w || !h) return;
    dpr = Math.min(2, window.devicePixelRatio || 1);
    canvas.width = Math.max(1, Math.round(w * dpr)); canvas.height = Math.max(1, Math.round(h * dpr));
    computeGeometry(w, h);
    if (V.ring) {
      if (edit) { V.ring.cx = w / 2; V.ring.cy = h / 2; }
      else { const m = G.R2 + G.r + 10; V.ring.cx = w > 2 * m ? clamp(V.ring.cx, m, w - m) : w / 2; V.ring.cy = h > 2 * m ? clamp(V.ring.cy, m, h - m) : h / 2; }
    }
    if (V.label.target && itemOf(V.label.target)) setLabelContent(V.label.target);
    kick();
  }
  function rebuild() {
    SL = o.getSlots();
    V.folders = V.folders.filter(f => SL[f.slot]?.type === 'folder');
    if (!edit && state.folder !== null && SL[state.folder]?.type !== 'folder') state = { ...state, folder: null, hover: null };
    buildIcons();
    const lt = V.label.target;
    if (lt && itemOf(lt)) setLabelContent(lt); else if (lt) V.label = { ...V.label, key: null, target: null, last: null };
    if (edit) {
      const h = E.hover;
      if (h && h.kind !== 'center' && h.kind !== 'ghost' && (!itemOf(h) || (h.kind !== 'child' && SL[h.slot].type !== h.kind))) E.hover = null;
      editSync(performance.now());
    }
    kick();
  }
  function showFolder(slot) {
    const now = performance.now();
    if (openFolderSlot() === slot) return;
    V.folders.forEach(f => { if (!f.closing) { f.closing = true; f.tc = now; } });
    if (slot !== null && SL[slot]?.type === 'folder') V.folders.push({ slot, t0: now, closing: false, tc: 0 });
    editSync(now); kick();
  }
  const selFolder = () => (E.sel && SL[E.sel.slot] && (E.sel.index != null || SL[E.sel.slot].type === 'folder') ? E.sel.slot : null);

  GL = initGL();
  if (!GL) ctx2d = canvas.getContext('2d');
  new ResizeObserver(resize).observe(root);
  buildIcons();

  return {
    rebuild, resize, kick,
    /** The page is on screen: the ring opens, with its animation. */
    show() {
      resize();
      if (edit) {
        const w = root.clientWidth, h = root.clientHeight;
        if (!w) return;
        startRing(w / 2, h / 2, performance.now(), true);
        const fs = selFolder();
        if (fs !== null) V.folders.push({ slot: fs, t0: performance.now(), closing: false, tc: 0 });
        editSync(performance.now());
        kick();
      } else if (!V.ring && !demoBusy) { pointer = { x: -9999, y: -9999 }; openCenter(true); }
    },
    setSelection(sel) { E.sel = sel; if (E.peek === null) showFolder(selFolder()); kick(); },
    peekFolder(slot) { E.peek = slot; showFolder(slot); },
    unpeek() { if (E.peek === null) return; E.peek = null; showFolder(selFolder()); },
    setDrop(t, ok) { E.drop = t; E.dropOk = ok; kick(); },
    hitTestClient(x, y) {
      const r = root.getBoundingClientRect();
      if (x < r.left || x > r.right || y < r.top || y > r.bottom) return { kind: 'none' };
      return hitTest({ x: x - r.left, y: y - r.top });
    },
    playDemo, replay,
  };
}
