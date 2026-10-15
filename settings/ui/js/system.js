// The Rust side of the window (src/main.rs), through Tauri.

export const invoke = (command, args) => window.__TAURI__.core.invoke(command, args);
export const listen = (event, fn) => window.__TAURI__.event.listen(event, fn);

/**
 * Asks for a file or a folder with the system's own dialog. `kind` is 'app',
 * 'file' or 'folder'. Resolves with the path, or null when cancelled.
 */
export const browse = kind => invoke('browse', { kind });
