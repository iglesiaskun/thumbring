// Apps: fullscreen apps, the apps where Thumbring stays off, and the
// profiles that give some apps gestures of their own.

import { $, esc, icon as drawIcon, openPop, closePop, toast } from '../ui.js';
import { change, store, undo } from '../store.js';
import { COMMAND_ICONS, commandHTML, commandShort } from '../commands.js';
import { BUILDER_ICONS, chooseCommand } from '../command-builder.js';
import { GESTURES, GESTURE_ICONS, GESTURE_LABELS } from './gestures.js';
import { appName, browse, runningApps } from '../system.js';

export const id = 'apps';
export const title = 'Apps';
export const icon = 'app-window';
export const icons = [...COMMAND_ICONS, ...BUILDER_ICONS, 'monitor', 'ban', 'layers', 'chevron-down', 'plus', 'x', 'arrow-up', 'arrow-down', 'trash-2', ...Object.values(GESTURE_ICONS)];

let root = null;
/** The profile that is open for editing, or -1. */
let openProfile = 0;

const profiles = () => store.settings.profiles ?? [];
const profileName = p => p.name || 'Untitled profile';

/** Problems the engine would warn about, to fix before saving. */
export function problems() {
  return profiles()
    .map((p, i) => (p.apps.length ? null : { message: `The profile “${profileName(p)}” needs at least one app.`, at: i }))
    .filter(Boolean);
}

/** Shows a profile that needs attention. */
export function reveal(problem) {
  openProfile = problem.at;
}

export function mount(pageRoot) {
  root = pageRoot;
  root.addEventListener('change', e => {
    if (e.target.id !== 'fullscreen') return;
    const value = e.target.checked;
    change(s => { s.apps.disable_in_fullscreen = value; });
    $('#fullscreen', root).focus();
  });
  root.addEventListener('input', e => {
    if (e.target.id !== 'pf-name') return;
    const i = Number(e.target.dataset.i), value = e.target.value;
    change(s => {
      if (value) s.profiles[i].name = value;
      else delete s.profiles[i].name;
    }, { key: `profile name ${i}`, render: false });
    const head = e.target.closest('.prof').querySelector('.prof-text .row-title');
    if (head) head.textContent = value || 'Untitled profile';
  });
  root.addEventListener('click', onClick);
}

export function render() {
  const a = store.settings.apps, list = profiles();
  if (openProfile >= list.length) openProfile = -1;
  root.innerHTML = `
    <header class="ph"><h1>${title}</h1><p>Where Thumbring stays out of the way, and apps that get gestures of their own.</p></header>
    <h2 class="sect">When an app is fullscreen</h2>
    <div class="card"><div class="row">
      <span class="row-ic">${drawIcon('monitor')}</span>
      <div class="row-text"><label class="row-title" for="fullscreen">Turn Thumbring off while an app is fullscreen</label>
        <span class="row-desc">Games, videos and presentations. The buttons then work as usual.</span></div>
      <input type="checkbox" class="switch" id="fullscreen" ${a.disable_in_fullscreen ? 'checked' : ''}>
    </div></div>
    <h2 class="sect">Excluded apps</h2>
    <div class="card">
      <div class="row">
        <span class="row-ic">${drawIcon('ban')}</span>
        <div class="row-text"><span class="row-title">In these apps, the buttons work as usual</span>
          <span class="row-desc">As if Thumbring weren’t running. Good for games, remote desktops and apps with their own use for the side buttons.</span></div>
        <button type="button" class="btn" data-act="ex-add">${drawIcon('plus')}Add app</button>
      </div>
      ${a.excluded.length
        ? `<ul class="applist">${a.excluded.map((app, i) => appRow(app, `<button type="button" class="btn subtle icon" data-act="ex-del" data-i="${i}" aria-label="Remove ${esc(app)}">${drawIcon('x')}</button>`)).join('')}</ul>`
        : '<p class="empty-note">No excluded apps.</p>'}
    </div>
    <h2 class="sect">Profiles</h2>
    <p class="sect-desc">A profile gives some apps gestures of their own. It changes only the gestures you set; the others come from Gestures. If an app is in two profiles, the first one wins.</p>
    <div class="stack">
      ${list.map(profileCard).join('') || '<div class="card"><p class="empty-note">No profiles. Every app uses the gestures from Gestures.</p></div>'}
      <button type="button" class="btn start" data-act="pf-new">${drawIcon('plus')}New profile</button>
    </div>`;
}

const appRow = (app, button) =>
  `<li class="app-row"><span class="avatar">${esc(app.charAt(0))}</span><span class="app-name">${esc(app)}</span>${button}</li>`;

function profileCard(p, i) {
  const open = i === openProfile, gestures = p.gestures ?? {};
  const set = GESTURES.filter(k => gestures[k] != null);
  const summary = set.length
    ? set.map(k => `${GESTURE_LABELS[k].replace('Drag ', '')}: ${commandShort(gestures[k])}`).join(' · ')
    : 'No gestures changed yet';
  return `<div class="card prof ${open ? 'open' : ''}">
    <button type="button" class="prof-head" data-act="pf-toggle" data-i="${i}" aria-expanded="${open}">
      <span class="row-ic">${drawIcon('layers')}</span>
      <span class="prof-text"><span class="row-title">${esc(profileName(p))}</span><span class="row-desc">${esc(p.apps.join(', ') || 'No apps yet')} · ${esc(summary)}</span></span>
      ${drawIcon('chevron-down', 'chev')}
    </button>
    ${open ? `<div class="prof-body">
      <div class="field"><label class="flabel" for="pf-name">Name</label>
        <input class="input" id="pf-name" data-i="${i}" value="${esc(p.name ?? '')}" placeholder="Browsers" autocomplete="off" spellcheck="false">
        <small class="fhint">For you and for the log.</small></div>
      <div class="field"><span class="flabel">Apps</span>
        <div class="chipset">${p.apps.map((app, j) => `<span class="tag">${esc(app)}<button type="button" data-act="pf-app-del" data-i="${i}" data-j="${j}" aria-label="Remove ${esc(app)}">${drawIcon('x')}</button></span>`).join('')}
          <button type="button" class="btn sm" data-act="pf-app-add" data-i="${i}">${drawIcon('plus')}Add app</button></div>
        ${p.apps.length ? '' : '<small class="fhint bad">A profile needs at least one app.</small>'}</div>
      <div class="field"><span class="flabel">Gestures in these apps</span>
        <div class="ovlist">${GESTURES.map(k => `<div class="ov-row"><span class="ov-dir">${drawIcon(GESTURE_ICONS[k])}${GESTURE_LABELS[k]}</span>
          <button type="button" class="cmd-btn ${gestures[k] == null ? 'inherit' : ''}" data-act="pf-gesture" data-i="${i}" data-k="${k}">${gestures[k] == null
            ? `<span class="cmd inherit"><span class="cmd-main"><span>Same as Gestures</span><span class="cmd-sub">${esc(commandShort(store.settings.gestures[k]))}</span></span></span>`
            : commandHTML(gestures[k], k === 'click' ? 'tap' : 'gesture')}${drawIcon('chevron-right', 'go')}</button></div>`).join('')}</div></div>
      <div class="prof-foot">
        <button type="button" class="btn sm" data-act="pf-up" data-i="${i}" ${i === 0 ? 'disabled' : ''}>${drawIcon('arrow-up')}Move up</button>
        <button type="button" class="btn sm" data-act="pf-down" data-i="${i}" ${i === profiles().length - 1 ? 'disabled' : ''}>${drawIcon('arrow-down')}Move down</button>
        <span class="spacer"></span>
        <button type="button" class="btn sm danger" data-act="pf-del" data-i="${i}">${drawIcon('trash-2')}Delete profile</button>
      </div>
    </div>` : ''}
  </div>`;
}

/** Where an app already is, so the same app does not end up in two places the engine would refuse. */
function placeOf(app) {
  if (store.settings.apps.excluded.includes(app)) return 'the excluded apps';
  const i = profiles().findIndex(p => p.apps.includes(app));
  return i >= 0 ? `the profile “${profileName(profiles()[i])}”` : null;
}

async function onClick(e) {
  const b = e.target.closest('[data-act]');
  if (!b) return;
  const a = b.dataset.act, i = Number(b.dataset.i);
  const refocus = selector => root.querySelector(selector)?.focus();
  if (a === 'ex-add') {
    pickApp(b, app => change(s => { s.apps.excluded.push(app); }));
  } else if (a === 'ex-del') {
    const app = store.settings.apps.excluded[i];
    change(s => { s.apps.excluded.splice(i, 1); });
    toast(`${app} removed`, { action: 'Undo', run: undo });
  } else if (a === 'pf-toggle') {
    openProfile = openProfile === i ? -1 : i;
    render();
    refocus(`[data-act="pf-toggle"][data-i="${i}"]`);
  } else if (a === 'pf-new') {
    openProfile = profiles().length;
    change(s => { (s.profiles ??= []).push({ apps: [], gestures: {} }); });
    refocus('#pf-name');
  } else if (a === 'pf-del') {
    const name = profileName(profiles()[i]);
    openProfile = -1;
    change(s => { s.profiles.splice(i, 1); });
    toast(`Deleted “${name}”`, { action: 'Undo', run: undo });
  } else if (a === 'pf-up' || a === 'pf-down') {
    const to = i + (a === 'pf-up' ? -1 : 1);
    openProfile = to;
    change(s => { s.profiles.splice(to, 0, s.profiles.splice(i, 1)[0]); });
    refocus(`[data-act="${a}"][data-i="${to}"]`);
  } else if (a === 'pf-app-add') {
    pickApp(b, app => change(s => { s.profiles[i].apps.push(app); }));
  } else if (a === 'pf-app-del') {
    const j = Number(b.dataset.j);
    change(s => { s.profiles[i].apps.splice(j, 1); });
  } else if (a === 'pf-gesture') {
    const k = b.dataset.k, p = profiles()[i];
    const value = await chooseCommand({
      title: `${GESTURE_LABELS[k]} in “${profileName(p)}”`, value: p.gestures?.[k] ?? null,
      mode: 'profile', ctx: k === 'click' ? 'tap' : 'gesture', inherit: store.settings.gestures[k], browse,
    });
    if (value === undefined) return;
    change(s => {
      const gestures = (s.profiles[i].gestures ??= {});
      if (value === null) delete gestures[k]; else gestures[k] = value;
    });
  }
}

/** Asks for an app: one of those open now, or a name typed in. */
async function pickApp(anchor, onPick) {
  const running = await runningApps();
  const add = async text => {
    const app = await appName(text);
    if (!app) return;
    closePop();
    const place = placeOf(app);
    if (place) { toast(`${app} is already in ${place}`); return; }
    onPick(app);
  };
  const free = running.filter(r => !placeOf(r.exe));
  const html = `<p class="pop-title">Add an app</p>
    <div class="inrow"><input class="input mono" id="app-typed" placeholder="app.exe" spellcheck="false" autocomplete="off" aria-label="App .exe name"><button type="button" class="btn primary" id="app-typed-add">Add</button></div>
    <p class="pop-note">Open now</p>
    <ul class="runlist">${free.map(r => `<li><button type="button" data-exe="${esc(r.exe)}"><span class="avatar">${esc(r.exe.charAt(0))}</span><span class="t"><span>${esc(r.title)}</span><small>${esc(r.exe)}</small></span></button></li>`).join('')
      || '<li class="pop-note">No other app is open, or they are all in a list already.</li>'}</ul>`;
  openPop(anchor, html, el => {
    el.addEventListener('click', e => {
      const b = e.target.closest('[data-exe]');
      if (b) add(b.dataset.exe);
      else if (e.target.closest('#app-typed-add')) add($('#app-typed', el).value);
    });
    el.addEventListener('keydown', e => {
      if (e.key === 'Enter' && e.target.id === 'app-typed') { e.preventDefault(); add(e.target.value); }
    });
  });
}
