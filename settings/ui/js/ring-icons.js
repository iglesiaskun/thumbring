// The icons a ring item can use: the engine's own Lucide files, which the
// Rust side sends once at start (src/icons.rs), so the window draws exactly
// what the ring will.

import { esc, svg, svgInner } from './ui.js';

const ringIcons = new Map();

export async function loadRingIcons(invoke) {
  for (const [name, text] of await invoke('ring_icons')) ringIcons.set(name, svgInner(text));
}

/** The names, in the engine's order. */
export const ringIconNames = () => [...ringIcons.keys()];

export const hasRingIcon = name => ringIcons.has(name);

export const ringSvg = (name, cls = '') => svg(ringIcons.get(name) ?? '', cls);

const firstLetter = name => ((name ?? '').trim().charAt(0) || '?').toUpperCase();

/** An item as the ring shows it: its icon, or the first letter of its name. */
export const itemGlyph = item =>
  item?.icon && ringIcons.has(item.icon) ? ringSvg(item.icon) : `<span class="letter">${esc(firstLetter(item?.name))}</span>`;
