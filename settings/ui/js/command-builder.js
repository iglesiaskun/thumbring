// The command builder: a dialog that asks what a gesture or a ring action
// does, and writes it in the text format of the settings file.

import { $, esc, icon, openDialog } from './ui.js';
import { ringSvg } from './ring-icons.js';
import { BUILTINS, MEDIA_KEYS, MOD_ORDER, commandShort, formatCommand, keyChips, nativeText, openKind, parseCommand, tomlString } from './commands.js';

export const BUILDER_ICONS = ['keyboard', 'music', 'external-link', 'sparkles', 'ellipsis', 'link', 'mouse', 'ban', 'square', 'arrow-left', 'arrow-right', 'rotate-cw'];

const TABS = [['keys', 'Shortcut', 'keyboard'], ['media', 'Media and volume', 'music'], ['open', 'Open', 'external-link'], ['builtin', 'Built-in', 'sparkles'], ['other', 'Other', 'ellipsis']];
const KEY_GROUPS = [
  ['Letters', 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('')],
  ['Digits', '0123456789'.split('')],
  ['Function keys', Array.from({ length: 24 }, (_, i) => 'F' + (i + 1))],
  ['Navigation', ['Left', 'Right', 'Up', 'Down', 'Home', 'End', 'PageUp', 'PageDown', 'Tab']],
  ['Editing', ['Enter', 'Esc', 'Space', 'Backspace', 'Delete', 'Insert']],
  ['Other', ['PrintScreen', 'Apps', 'Plus', 'Minus', '.', ',', ';', '/', '\\', '[', ']', '=', '`', "'"]],
];
const OPEN_KINDS = [
  ['app', 'App', 'App or command', 'notepad.exe or C:\\Program Files\\App\\app.exe'],
  ['file', 'File', 'File', '%USERPROFILE%\\Documents\\report.pdf'],
  ['folder', 'Folder', 'Folder', '%USERPROFILE%\\Downloads'],
  ['link', 'Website or link', 'Address', 'https://example.com'],
];
const MODIFIER_KEYS = ['Control', 'Alt', 'Shift', 'Meta', 'OS', 'AltGraph'];

/**
 * Asks what a command does. `mode` is 'ring', 'gesture' or 'profile'; `ctx`
 * says what "native" means there (see `nativeText`); `inherit` is the
 * command a profile gesture gets when left as in Gestures. Resolves with the
 * new text, `null` for "Same as Gestures", or `undefined` when cancelled.
 */
export function chooseCommand({ title, value, mode, ctx = mode === 'ring' ? 'ring' : 'gesture', inherit, browse }) {
  const st = { tab: 'keys', mods: [], key: '', media: 'PlayPause', openKind: 'app', target: '', args: '', builtin: 'screenshot', other: mode === 'ring' ? 'none' : 'native', recording: false, live: null };
  if (value == null) {
    if (mode === 'profile') { st.tab = 'other'; st.other = 'inherit'; }
  } else {
    const c = parseCommand(value);
    if (c.kind === 'keys') {
      if (!c.mods.length && MEDIA_KEYS[c.key]) { st.tab = 'media'; st.media = c.key; } else { st.mods = c.mods; st.key = c.key; }
    } else if (c.kind === 'open') { st.tab = 'open'; st.target = c.target; st.args = c.args; st.openKind = openKind(c.target); }
    else if (c.kind === 'builtin') { st.tab = 'builtin'; st.builtin = c.id; }
    else if (c.kind === 'native' || c.kind === 'none') { st.tab = 'other'; st.other = c.kind; }
  }

  const result = () => {
    switch (st.tab) {
      case 'keys': return st.key ? formatCommand({ kind: 'keys', mods: st.mods, key: st.key }) : undefined;
      case 'media': return st.media;
      case 'open': {
        const t = st.target.trim();
        return t ? formatCommand({ kind: 'open', target: t, args: st.openKind === 'app' ? st.args.trim() : '' }) : undefined;
      }
      case 'builtin': return st.builtin;
      default: return st.other === 'inherit' ? null : st.other;
    }
  };
  const note = () => {
    const v = result();
    if (v === undefined) return `<span class="bad">${st.tab === 'keys' ? 'Record a shortcut or pick a key' : 'Type what to open'}</span>`;
    if (v === null) return 'In the file: this gesture is left out of the profile';
    return `In the file: <code>${esc(tomlString(v))}</code>`;
  };
  const buttons = () => [
    { label: 'Cancel', value: 'cancel' },
    { label: 'Done', primary: true, value: 'done', disabled: result() === undefined },
  ];
  const body = () => `
    <div class="cb-tabs" role="tablist" aria-label="Kind of command">${TABS.map(([k, l, ic]) =>
      `<button type="button" class="cb-tab" role="tab" id="cbt-${k}" aria-selected="${st.tab === k}" tabindex="${st.tab === k ? 0 : -1}" data-tab="${k}">${icon(ic)}${l}</button>`).join('')}</div>
    <div class="cb-panel" role="tabpanel" aria-labelledby="cbt-${st.tab}">${panel()}</div>`;
  const panel = () => {
    if (st.tab === 'keys') {
      const shown = st.recording
        ? (st.live?.length ? keyChips(st.live, '') + '<span class="plus">+ …</span>' : '<span class="muted">Press the shortcut now. Esc cancels.</span>')
        : st.key ? keyChips(st.mods, st.key) : '<span class="muted">No shortcut yet</span>';
      const all = KEY_GROUPS.flatMap(g => g[1]);
      return `<div class="recbox ${st.recording ? 'rec' : ''}"><div aria-live="polite">${shown}</div>
          <button type="button" class="btn ${st.recording ? '' : 'primary'}" id="cb-rec">${st.recording ? 'Stop' : st.key ? 'Record again' : 'Record'}</button></div>
        <div class="field"><span class="flabel">Or build it</span>
          <div class="modrow">${MOD_ORDER.map(m => `<button type="button" class="btn mod" data-mod="${m}" aria-pressed="${st.mods.includes(m)}">${m}</button>`).join('<span class="plus">+</span>')}<span class="plus">+</span>
            <select class="input keysel" id="cb-key" aria-label="Key">
              <option value="" ${st.key ? '' : 'selected'}>Pick a key</option>
              ${st.key && !all.includes(st.key) ? `<option value="${esc(st.key)}" selected>${esc(st.key)}</option>` : ''}
              ${KEY_GROUPS.map(([g, ks]) => `<optgroup label="${g}">${ks.map(k => `<option value="${esc(k)}" ${k === st.key ? 'selected' : ''}>${esc(k)}</option>`).join('')}</optgroup>`).join('')}
            </select></div></div>
        <p class="hint-p">Windows reacts to some Win shortcuts before any app sees them, so those can’t always be recorded: turn on Win and pick the key. Win+L can’t be sent at all; use Lock screen, in Built-in.</p>`;
    }
    if (st.tab === 'media') {
      return `<div class="tiles">${Object.entries(MEDIA_KEYS).map(([k, m]) =>
        `<button type="button" class="tile" data-media="${k}" aria-pressed="${st.media === k}">${m.ring ? ringSvg(m.ring) : icon(m.ui)}<span>${m.label}</span></button>`).join('')}</div>
        <p class="hint-p">They work like the keys of a media keyboard, in whatever app is playing.</p>`;
    }
    if (st.tab === 'open') {
      const k = OPEN_KINDS.find(x => x[0] === st.openKind);
      return `<div class="seg" role="radiogroup" aria-label="What to open">${OPEN_KINDS.map(([id, l]) =>
          `<input type="radio" name="cb-open" id="cbo-${id}" value="${id}" ${st.openKind === id ? 'checked' : ''}><label for="cbo-${id}">${l}</label>`).join('')}</div>
        <div class="field"><label class="flabel" for="cb-target">${k[2]}</label>
          <div class="inrow"><input class="input mono" id="cb-target" value="${esc(st.target)}" placeholder="${esc(k[3])}" spellcheck="false" autocomplete="off">
          ${st.openKind !== 'link' && browse ? '<button type="button" class="btn" id="cb-browse">Browse…</button>' : ''}</div></div>
        ${st.openKind === 'app' ? `<div class="field"><label class="flabel" for="cb-args">Arguments <span class="opt">optional</span></label>
          <input class="input mono" id="cb-args" value="${esc(st.args)}" placeholder="--new-window" spellcheck="false" autocomplete="off"></div>` : ''}
        <p class="hint-p">Variables such as %USERPROFILE% work. ${st.openKind === 'link'
          ? 'Any address Windows knows how to open works too, such as ms-settings: or mailto:.'
          : 'What it opens runs without administrator rights, even if Thumbring has them.'}</p>`;
    }
    if (st.tab === 'builtin') {
      return `<div class="tiles">${Object.entries(BUILTINS).map(([k, b]) =>
        `<button type="button" class="tile" data-builtin="${k}" aria-pressed="${st.builtin === k}">${ringSvg(b.ring)}<span>${b.label}</span><small>${b.desc}</small></button>`).join('')}</div>`;
    }
    const list = [];
    if (mode === 'profile') list.push(['inherit', 'Same as Gestures', `${commandShort(inherit)}, like in every other app`, 'link']);
    if (mode !== 'ring') list.push(['native', 'Usual function', nativeText(ctx), 'mouse']);
    list.push(['none', 'Nothing', mode === 'ring' ? 'The item shows up but does nothing' : 'Turns this gesture off', 'ban']);
    return `<div class="tiles">${list.map(([k, l, d, ic]) =>
      `<button type="button" class="tile" data-other="${k}" aria-pressed="${st.other === k}">${icon(ic)}<span>${l}</span><small>${esc(d)}</small></button>`).join('')}</div>`;
  };

  const dialog = openDialog({ title, body: body(), buttons: buttons(), note: note(), wide: true });
  const root = dialog.root;
  // Redraws the parts that change; `focus` is a selector to focus after.
  const redraw = focus => {
    dialog.update({ body: body(), buttons: buttons(), note: note() });
    if (focus) $(focus, root)?.focus();
  };
  const refreshFoot = () => dialog.update({ buttons: buttons(), note: note() });

  root.addEventListener('click', e => {
    const b = e.target.closest('button');
    if (!b || !root.contains(b) || b.closest('.dlg-buttons')) return;
    if (b.dataset.tab) { st.tab = b.dataset.tab; st.recording = false; redraw(`#cbt-${st.tab}`); }
    else if (b.id === 'cb-rec') { st.recording = !st.recording; st.live = null; redraw('#cb-rec'); }
    else if (b.dataset.mod) {
      const m = b.dataset.mod;
      st.mods = st.mods.includes(m) ? st.mods.filter(x => x !== m) : MOD_ORDER.filter(x => x === m || st.mods.includes(x));
      redraw(`[data-mod="${m}"]`);
    } else if (b.dataset.media) { st.media = b.dataset.media; redraw(`[data-media="${st.media}"]`); }
    else if (b.dataset.builtin) { st.builtin = b.dataset.builtin; redraw(`[data-builtin="${st.builtin}"]`); }
    else if (b.dataset.other) { st.other = b.dataset.other; redraw(`[data-other="${st.other}"]`); }
    else if (b.id === 'cb-browse') {
      browse(st.openKind).then(path => {
        if (!path) return;
        st.target = path;
        $('#cb-target', root).value = path;
        refreshFoot();
      });
    }
  });
  root.addEventListener('change', e => {
    if (e.target.id === 'cb-key') { st.key = e.target.value; refreshFoot(); }
    else if (e.target.name === 'cb-open') { st.openKind = e.target.value; redraw('#cb-target'); }
  });
  root.addEventListener('input', e => {
    if (e.target.id === 'cb-target') { st.target = e.target.value; refreshFoot(); }
    else if (e.target.id === 'cb-args') { st.args = e.target.value; refreshFoot(); }
  });
  root.addEventListener('keydown', e => {
    if (!e.target.dataset?.tab || (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft')) return;
    const ks = TABS.map(t => t[0]), i = ks.indexOf(st.tab);
    st.tab = ks[(i + (e.key === 'ArrowRight' ? 1 : ks.length - 1)) % ks.length];
    st.recording = false;
    e.preventDefault();
    redraw(`#cbt-${st.tab}`);
  });

  // While recording, every key goes to the recorder, before anything else.
  const modsOf = e => MOD_ORDER.filter(m => (m === 'Ctrl' && e.ctrlKey) || (m === 'Alt' && e.altKey) || (m === 'Shift' && e.shiftKey) || (m === 'Win' && e.metaKey));
  const onKeyDown = e => {
    if (!st.recording) return;
    e.preventDefault();
    e.stopPropagation();
    const mods = modsOf(e);
    if (MODIFIER_KEYS.includes(e.key)) { st.live = mods; redraw(); return; }
    if (e.key === 'Escape' && !mods.length) { st.recording = false; st.live = null; redraw('#cb-rec'); return; }
    const key = keyFromEvent(e);
    if (!key) return;
    st.recording = false;
    st.live = null;
    if (!mods.length && MEDIA_KEYS[key]) { st.tab = 'media'; st.media = key; redraw(`[data-media="${key}"]`); return; }
    st.mods = mods;
    st.key = key;
    redraw('#cb-rec');
  };
  const onKeyUp = e => {
    if (!st.recording) return;
    e.preventDefault();
    e.stopPropagation();
    st.live = modsOf(e);
    redraw();
  };
  window.addEventListener('keydown', onKeyDown, true);
  window.addEventListener('keyup', onKeyUp, true);

  return dialog.done.then(answer => {
    window.removeEventListener('keydown', onKeyDown, true);
    window.removeEventListener('keyup', onKeyUp, true);
    return answer === 'done' ? result() : undefined;
  });
}

/** The key of a key press, by its place on the keyboard (not the character it types). */
function keyFromEvent(e) {
  const c = e.code;
  if (/^Key[A-Z]$/.test(c)) return c.slice(3);
  if (/^Digit\d$/.test(c)) return c.slice(5);
  if (/^Numpad\d$/.test(c)) return c.slice(6);
  if (/^F([1-9]|1\d|2[0-4])$/.test(c)) return c;
  const byCode = {
    ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down', Tab: 'Tab', Enter: 'Enter', NumpadEnter: 'Enter', Escape: 'Esc',
    Space: 'Space', Backspace: 'Backspace', Delete: 'Delete', Insert: 'Insert', Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
    PrintScreen: 'PrintScreen', ContextMenu: 'Apps', Minus: 'Minus', NumpadSubtract: 'Minus', NumpadAdd: 'Plus', Equal: '=', Period: '.', Comma: ',',
    Slash: '/', Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']', Backslash: '\\', Backquote: '`',
    MediaPlayPause: 'PlayPause', MediaTrackNext: 'NextTrack', MediaTrackPrevious: 'PrevTrack', MediaStop: 'MediaStop',
    AudioVolumeUp: 'VolumeUp', AudioVolumeDown: 'VolumeDown', AudioVolumeMute: 'VolumeMute',
    BrowserBack: 'BrowserBack', BrowserForward: 'BrowserForward', BrowserRefresh: 'BrowserRefresh',
  };
  if (byCode[c]) return byCode[c];
  // Media keys sometimes come with an empty code.
  const byKey = { MediaPlayPause: 'PlayPause', MediaTrackNext: 'NextTrack', MediaTrackPrevious: 'PrevTrack', MediaStop: 'MediaStop', AudioVolumeUp: 'VolumeUp', AudioVolumeDown: 'VolumeDown', AudioVolumeMute: 'VolumeMute' };
  if (byKey[e.key]) return byKey[e.key];
  return e.key?.length === 1 ? e.key.toUpperCase() : null;
}
