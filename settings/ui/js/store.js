// The settings being edited, with undo and redo.
//
// `store.settings` has the shape of the settings file. The pages read it
// freely and change it only through `change`, which keeps the history and
// tells the window to draw again.

export const store = {
  settings: null,
  /** The default settings, in the same shape. */
  defaults: null,
  /** Allowed range of each number, by key path: { 'behavior.hold_to_open_ms': [60, 1000] }. */
  limits: {},
  /** The blob shape each style starts from: { viscous: {...}, elastic: {...} }. */
  blobPresets: {},
  /** Problems the engine found in the file when it was read. */
  warnings: [],
  path: '',
  /** The file does not exist yet: saving creates it. */
  newFile: false,
};

let saved = '';
const undoList = [], redoList = [];
let lastKey = null, lastAt = 0;
const listeners = new Set();

/** The same text for equal settings, whatever the order of their keys. */
function stable(value) {
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  if (value && typeof value === 'object') {
    // A key set to null is the same as a missing key: saving leaves it out.
    return `{${Object.keys(value).filter(k => value[k] != null).sort().map(k => `${JSON.stringify(k)}:${stable(value[k])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

/** Starts over from what the file says. */
export function reset(snapshot) {
  store.settings = snapshot.settings;
  store.defaults = snapshot.defaults;
  store.limits = snapshot.limits;
  store.blobPresets = snapshot.blob_presets;
  store.warnings = snapshot.warnings;
  store.path = snapshot.path;
  store.newFile = snapshot.new_file;
  saved = stable(snapshot.settings);
  undoList.length = 0;
  redoList.length = 0;
  lastKey = null;
  notify(true);
}

/**
 * Changes the settings. Changes with the same `key` less than 1.5 s apart
 * (a slider being dragged, a name being typed) undo together. With
 * `render: false`, the page keeps its controls (and their focus) and
 * updates them itself.
 */
export function change(fn, { key = null, render = true } = {}) {
  const now = performance.now();
  if (!(key && key === lastKey && now - lastAt < 1500)) {
    undoList.push(JSON.stringify(store.settings));
    if (undoList.length > 200) undoList.shift();
    redoList.length = 0;
  }
  lastKey = key;
  lastAt = now;
  fn(store.settings);
  notify(render);
}

export const dirty = () => stable(store.settings) !== saved;
export const canUndo = () => undoList.length > 0;
export const canRedo = () => redoList.length > 0;

export function undo() {
  if (!undoList.length) return false;
  redoList.push(JSON.stringify(store.settings));
  store.settings = JSON.parse(undoList.pop());
  lastKey = null;
  notify(true);
  return true;
}

export function redo() {
  if (!redoList.length) return false;
  undoList.push(JSON.stringify(store.settings));
  store.settings = JSON.parse(redoList.pop());
  lastKey = null;
  notify(true);
  return true;
}

/** `fn(render)` runs after every change. */
export function onChange(fn) {
  listeners.add(fn);
}

function notify(render) {
  for (const fn of listeners) fn(render);
}

/** The value at a key path such as 'behavior.hold_to_open_ms'. */
export const get = (object, path) => path.split('.').reduce((o, k) => o?.[k], object);

export function set(object, path, value) {
  const keys = path.split('.'), last = keys.pop();
  keys.reduce((o, k) => o[k], object)[last] = value;
}
