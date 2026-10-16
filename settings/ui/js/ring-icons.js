// The icons a ring item can use: the engine's own Lucide files, which the
// Rust side sends once at start (src/icons.rs), so the window draws exactly
// what the ring will, and the SVG or PNG files of the user's own.

import { esc, svg, svgInner } from './ui.js';

const ringIcons = new Map();
/** Icon files by path: their markup, or null while loading or when unusable. */
const fileIcons = new Map();
let call = null;

export async function loadRingIcons(invoke) {
  call = invoke;
  for (const [name, text] of await invoke('ring_icons')) ringIcons.set(name, svgInner(text));
}

/** The names, in the engine's order. */
export const ringIconNames = () => [...ringIcons.keys()];

export const hasRingIcon = name => ringIcons.has(name);

export const ringSvg = (name, cls = '') => svg(ringIcons.get(name) ?? '', cls);

/** An icon written as the path of an .svg or .png file. */
export const isIconFile = icon => !!icon && !ringIcons.has(icon) && /\.(svg|png)$/i.test(icon);

/** The file name of an icon file, for short labels. */
export const iconFileName = path => path.split(/[\\/]/).pop();

const firstLetter = name => ((name ?? '').trim().charAt(0) || '?').toUpperCase();

/** An item as the ring shows it: its icon, or the first letter of its name. */
export function itemGlyph(item) {
  if (item?.icon && ringIcons.has(item.icon)) return ringSvg(item.icon);
  if (isIconFile(item?.icon)) {
    const drawn = fileGlyph(item.icon);
    if (drawn) return drawn;
  }
  return `<span class="letter">${esc(firstLetter(item?.name))}</span>`;
}

/** Forgets the icon files read, so a changed picture is read again. */
export function forgetIconFiles() {
  fileIcons.clear();
}

/**
 * The markup of an icon file, or null until it is read. Once read, the
 * window gets a 'thumbring:icons' event to draw it.
 */
function fileGlyph(path) {
  if (fileIcons.has(path)) return fileIcons.get(path);
  fileIcons.set(path, null);
  call?.('icon_file', { path }).then(file => {
    fileIcons.set(path, file.format === 'svg' ? userSvg(file.text) : `<img class="png" alt="" src="data:image/png;base64,${base64(file.bytes)}">`);
    window.dispatchEvent(new Event('thumbring:icons'));
  }, () => { /* the item shows its first letter, as in the engine */ });
  return null;
}

/**
 * An SVG of the user's own, ready to sit in the page. As in the engine,
 * currentColor and the shapes that set no color take the icon color. What
 * could run (scripts, event attributes, links) is taken out.
 */
function userSvg(text) {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const root = doc.documentElement;
  if (root.nodeName !== 'svg' || doc.querySelector('parsererror')) return null;
  for (const el of root.querySelectorAll('script, foreignObject')) el.remove();
  for (const el of [root, ...root.querySelectorAll('*')]) {
    for (const { name, value } of [...el.attributes]) {
      // Links to other files go; links inside the picture (#id) stay.
      if (/^on/i.test(name) || (/href$/i.test(name) && !value.startsWith('#'))) el.removeAttribute(name);
    }
  }
  if (!root.hasAttribute('fill')) root.setAttribute('fill', 'currentColor');
  if (!root.hasAttribute('viewBox') && root.hasAttribute('width') && root.hasAttribute('height')) {
    root.setAttribute('viewBox', `0 0 ${parseFloat(root.getAttribute('width'))} ${parseFloat(root.getAttribute('height'))}`);
  }
  root.removeAttribute('width');
  root.removeAttribute('height');
  root.setAttribute('class', 'ui user-icon');
  root.setAttribute('aria-hidden', 'true');
  return new XMLSerializer().serializeToString(root);
}

function base64(bytes) {
  let text = '';
  for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.slice(i, i + 0x8000));
  return btoa(text);
}
