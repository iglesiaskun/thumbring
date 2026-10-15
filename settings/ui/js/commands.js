// Commands, in the text format of the settings file (core/src/command.rs):
// "Ctrl+Shift+T", "VolumeUp", "open:C:\folder", "screenshot", "native",
// "none". The engine has the last word: saving checks the file with its own
// reader. This copy only reads and writes the text for the interface.

import { esc, icon } from './ui.js';
import { ringSvg } from './ring-icons.js';
import { store } from './store.js';

export const MOD_ORDER = ['Ctrl', 'Win', 'Alt', 'Shift'];
const MOD_NAMES = { ctrl: 'Ctrl', control: 'Ctrl', win: 'Win', windows: 'Win', alt: 'Alt', shift: 'Shift' };
// The first name of each key is the one written in the file; the others are aliases.
const NAMED_KEYS = [['Left'], ['Right'], ['Up'], ['Down'], ['Tab'], ['Enter', 'Return'], ['Esc', 'Escape'], ['Space'], ['Backspace'], ['Delete', 'Del'], ['Insert', 'Ins'], ['Home'], ['End'], ['PageUp', 'PgUp'], ['PageDown', 'PgDn'], ['PrintScreen', 'PrtSc'], ['Apps', 'Menu'], ['VolumeUp'], ['VolumeDown'], ['VolumeMute', 'Mute'], ['PlayPause'], ['NextTrack'], ['PrevTrack', 'PreviousTrack'], ['MediaStop'], ['BrowserBack'], ['BrowserForward'], ['BrowserRefresh'], ['Plus'], ['Minus']];

/** Media and browser keys: keys without modifiers, shown with their own names. */
export const MEDIA_KEYS = {
  PlayPause: { label: 'Play or pause', ring: 'play' },
  NextTrack: { label: 'Next track', ring: 'skip-forward' },
  PrevTrack: { label: 'Previous track', ring: 'skip-back' },
  MediaStop: { label: 'Stop', ui: 'square' },
  VolumeUp: { label: 'Volume up', ring: 'volume-2' },
  VolumeDown: { label: 'Volume down', ring: 'volume-1' },
  VolumeMute: { label: 'Mute', ring: 'volume-x' },
  BrowserBack: { label: 'Browser back', ui: 'arrow-left' },
  BrowserForward: { label: 'Browser forward', ui: 'arrow-right' },
  BrowserRefresh: { label: 'Refresh', ui: 'rotate-cw' },
};

export const BUILTINS = {
  screenshot: { label: 'Screenshot', desc: 'Snipping tool, like Win+Shift+S', ring: 'scan' },
  emoji: { label: 'Emoji panel', desc: 'Like Win+.', ring: 'smile' },
  explorer: { label: 'File Explorer', desc: 'Like Win+E', ring: 'folder-open' },
  lock: { label: 'Lock screen', desc: 'Locks the session', ring: 'lock' },
};

/** What some common shortcuts do, shown under them. */
const KNOWN = {
  'Win+Tab': 'Task View', 'Win+D': 'Show the desktop', 'Ctrl+Win+Right': 'Next virtual desktop', 'Ctrl+Win+Left': 'Previous virtual desktop',
  'Ctrl+Tab': 'Next tab', 'Ctrl+Shift+Tab': 'Previous tab', 'Ctrl+W': 'Close the tab', 'Ctrl+Shift+T': 'Reopen the closed tab', 'Ctrl+T': 'New tab',
  'Alt+Left': 'Back', 'Alt+Right': 'Forward', 'F5': 'Refresh', 'Ctrl+C': 'Copy', 'Ctrl+V': 'Paste', 'Ctrl+X': 'Cut', 'Ctrl+Z': 'Undo', 'Ctrl+Y': 'Redo',
  'Win+V': 'Clipboard history', 'Win+Shift+S': 'Screenshot', 'Win+E': 'File Explorer', 'Win+I': 'Settings', 'Alt+Tab': 'Switch windows',
};

export const BUTTON_NAMES = { back: 'Back', forward: 'Forward', middle: 'Middle button' };

function keyName(text) {
  const t = text.trim();
  if (!t) return null;
  for (const names of NAMED_KEYS) if (names.some(n => n.toLowerCase() === t.toLowerCase())) return names[0];
  if ([...t].length === 1) return t.toUpperCase();
  const m = /^[Ff]([1-9]|1\d|2[0-4])$/.exec(t);
  return m ? 'F' + m[1] : null;
}

/** The text of a command as a value: { kind: 'keys' | 'open' | 'builtin' | 'native' | 'none' | 'error', ... }. */
export function parseCommand(text) {
  const t = String(text ?? '').trim();
  if (!t) return { kind: 'error', msg: 'Empty command' };
  const low = t.toLowerCase();
  if (low === 'native') return { kind: 'native' };
  if (low === 'none') return { kind: 'none' };
  if (BUILTINS[low]) return { kind: 'builtin', id: low };
  if (low.startsWith('open:')) {
    const rest = t.slice(5).trim();
    if (rest.startsWith('"')) {
      const end = rest.indexOf('"', 1);
      if (end < 0) return { kind: 'error', msg: 'The quoted path is missing its closing quote' };
      const target = rest.slice(1, end).trim();
      if (!target) return { kind: 'error', msg: 'Nothing to open' };
      return { kind: 'open', target, args: rest.slice(end + 1).trim() };
    }
    if (!rest) return { kind: 'error', msg: 'Nothing to open after "open:"' };
    return { kind: 'open', target: rest, args: '' };
  }
  const parts = t.split('+').map(s => s.trim());
  const last = parts.pop(), mods = [];
  for (const p of parts) {
    if (!p) return { kind: 'error', msg: 'The shortcut has an extra "+"' };
    const m = MOD_NAMES[p.toLowerCase()];
    if (!m) return { kind: 'error', msg: `"${p}" is not Ctrl, Win, Alt or Shift` };
    if (mods.includes(m)) return { kind: 'error', msg: `${m} appears twice` };
    mods.push(m);
  }
  if (!last || MOD_NAMES[last.toLowerCase()]) return { kind: 'error', msg: 'A key is missing after the modifiers' };
  const key = keyName(last);
  if (!key) return { kind: 'error', msg: `Unknown key "${last}"` };
  return { kind: 'keys', mods: MOD_ORDER.filter(m => mods.includes(m)), key };
}

/** The value back to text, as the settings file writes it. */
export function formatCommand(c) {
  switch (c.kind) {
    case 'native': return 'native';
    case 'none': return 'none';
    case 'builtin': return c.id;
    case 'open': return c.args ? `open:"${c.target}" ${c.args}` : `open:${c.target}`;
    case 'keys': return [...MOD_ORDER.filter(m => c.mods.includes(m)), c.key === '+' ? 'Plus' : c.key === '-' ? 'Minus' : c.key].join('+');
    default: return '';
  }
}

/** How the value looks in the file: single quotes keep backslashes as they are. */
export const tomlString = v => (/[\\"]/.test(v) && !v.includes("'") ? `'${v}'` : JSON.stringify(v));

export const keyChips = (mods, key) =>
  `<span class="keys">${[...mods, ...(key ? [key] : [])].map(k => `<kbd>${esc(k)}</kbd>`).join('<span class="plus">+</span>')}</span>`;

/** What `open:` points at: 'link', 'app', 'folder' or 'file'. */
export function openKind(target) {
  if (/^[a-z][a-z0-9+.-]+:/i.test(target) && !/^[a-z]:[\\/]/i.test(target)) return 'link';
  const last = target.replace(/[\\/]+$/, '').split(/[\\/]/).pop();
  if (/\.(exe|com|bat|cmd|msc|lnk)$/i.test(target) || (!/[\\/]/.test(target) && !/\.\w{2,4}$/.test(target))) return 'app';
  if (/[\\/]$/.test(target) || !/\.\w{1,5}$/.test(last)) return 'folder';
  return 'file';
}

const baseName = target => { const s = target.replace(/[\\/]+$/, ''); return s.split(/[\\/]/).pop() || s; };

/**
 * What "native" does: `ctx` is 'tap' for the quick tap of the gestures,
 * 'gesture' for a direction and 'ring' for a ring action.
 */
export function nativeText(ctx) {
  const name = BUTTON_NAMES[ctx === 'ring' ? store.settings.ring.button : store.settings.gestures.button];
  return ctx === 'tap' ? `${name}, as usual` : `The click of ${name}`;
}

/** A command as the interface shows it, with an icon and a line under it. */
export function commandHTML(text, ctx = 'ring') {
  const wrap = (ic, main, sub, cls = '') =>
    `<span class="cmd ${cls}">${ic}<span class="cmd-main"><span>${main}</span>${sub ? `<span class="cmd-sub">${sub}</span>` : ''}</span></span>`;
  if (text == null) return wrap(icon('circle-alert', 'cmd-ic'), 'No command yet', 'Choose what it does', 'warn');
  const c = parseCommand(text);
  switch (c.kind) {
    case 'keys': {
      const m = MEDIA_KEYS[c.key];
      if (!c.mods.length && m) return wrap(m.ring ? ringSvg(m.ring, 'cmd-ic') : icon(m.ui, 'cmd-ic'), esc(m.label), 'Media key');
      const known = KNOWN[formatCommand(c)];
      return wrap('', keyChips(c.mods, c.key), known ? esc(known) : '');
    }
    case 'open': {
      const k = openKind(c.target);
      const ic = { link: 'globe', folder: 'folder', app: 'app-window', file: 'file' }[k];
      return wrap(icon(ic, 'cmd-ic'), 'Open ' + esc(k === 'link' ? c.target : baseName(c.target)), esc(c.target + (c.args ? ' ' + c.args : '')));
    }
    case 'builtin': {
      const b = BUILTINS[c.id];
      return wrap(ringSvg(b.ring, 'cmd-ic'), esc(b.label), esc(b.desc));
    }
    case 'native': return wrap(icon('mouse', 'cmd-ic'), 'Usual function', esc(nativeText(ctx)));
    case 'none': return wrap(icon('ban', 'cmd-ic'), 'Nothing', ctx === 'ring' ? 'The item shows up but does nothing' : 'Turned off');
    default: return wrap(icon('circle-alert', 'cmd-ic'), 'Invalid command', esc(c.msg), 'err');
  }
}

/** A command in a few words, for labels and lists. */
export function commandShort(text) {
  if (text == null) return 'No command';
  const c = parseCommand(text);
  switch (c.kind) {
    case 'keys': return !c.mods.length && MEDIA_KEYS[c.key] ? MEDIA_KEYS[c.key].label : formatCommand(c);
    case 'open': return 'Opens ' + (openKind(c.target) === 'link' ? c.target : baseName(c.target));
    case 'builtin': return BUILTINS[c.id].label;
    case 'native': return 'Usual function';
    case 'none': return 'Nothing';
    default: return 'Invalid';
  }
}

export const COMMAND_ICONS = ['circle-alert', 'square', 'arrow-left', 'arrow-right', 'rotate-cw', 'globe', 'folder', 'app-window', 'file', 'mouse', 'ban'];
