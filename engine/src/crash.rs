//! What happens on a bug: the engine writes it to the log, gives the mouse
//! back and quits.
//!
//! A panic (a bug in Rust code) goes through [`install`], which also says so
//! in a message. Without it, a panic would end only the thread where it
//! happened. The others would keep running: the hook could keep taking the
//! gesture button with no worker left to act on it, and the release build
//! would show nothing.
//!
//! A crash in native code, such as an access violation inside a Windows DLL,
//! goes through [`install_exception_filter`]. Windows ends the whole process
//! anyway; without the filter, the log would stop at whatever came before,
//! with no hint of what happened or where.

use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ptr, slice, thread};

use windows::Wdk::System::Threading::{NtQueryInformationThread, ThreadNameInformation};
use windows::Win32::Foundation::{
    EXCEPTION_ACCESS_VIOLATION, EXCEPTION_ILLEGAL_INSTRUCTION, EXCEPTION_INT_DIVIDE_BY_ZERO,
    EXCEPTION_STACK_OVERFLOW, HMODULE, NTSTATUS, UNICODE_STRING,
};
use windows::Win32::System::Diagnostics::Debug::{
    EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS, LPTOP_LEVEL_EXCEPTION_FILTER,
    SetUnhandledExceptionFilter,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Threading::GetCurrentThread;
use windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR;
use windows::core::PCWSTR;

use crate::log::{self, Level, error};
use crate::{hook, text, tray};

/// The usual exit code of a Rust program that panicked.
const EXIT_CODE: i32 = 101;

/// Replaces the default panic handler. `log_file` goes in the message, so the
/// person knows where to look.
pub fn install(log_file: PathBuf) {
    std::panic::set_hook(Box::new(move |info| {
        let current = thread::current();
        let name = current.name().unwrap_or("unnamed");
        // `info` has the message and the file and line.
        error!("bug in the {name} thread, the engine is stopping: {info}");
        hook::give_up();
        tray::remove_now();
        crate::alert(&text::alert::crashed(&log_file), MB_ICONERROR);
        // Ends every thread at once; Windows removes whatever hook is left.
        std::process::exit(EXIT_CODE);
    }));
}

/// The filter that was in place before ours, if any; ours calls it after
/// doing its part.
static PREVIOUS: OnceLock<LPTOP_LEVEL_EXCEPTION_FILTER> = OnceLock::new();
/// The filter already ran. A second crash, inside the filter or on another
/// thread at the same time, goes straight on to Windows.
static CRASHED: AtomicBool = AtomicBool::new(false);

/// Writes a crash in native code to the log, gives the mouse back and lets
/// Windows end the process as it would without the filter, so Windows Error
/// Reporting can still save a dump. Under a debugger, the debugger gets the
/// crash instead.
pub fn install_exception_filter() {
    // SAFETY: `on_exception` is a plain function that lives for the whole
    // program.
    let previous = unsafe { SetUnhandledExceptionFilter(Some(on_exception)) };
    let _ = PREVIOUS.set(previous);
}

/// Windows calls this on the thread of an exception no code handled.
///
/// The process is in an unknown state, and the thread may hold any lock, the
/// heap's included. So this works on its own stack and takes no lock the
/// crashed code could hold: the log line goes through `write_unlocked`, the
/// mouse goes back through an atomic and a posted message, and the tray icon
/// comes out with one call to the shell, as in the panic handler. Only the
/// module lookup takes the loader's locks, which a crash inside the loader
/// itself could hold.
unsafe extern "system" fn on_exception(pointers: *const EXCEPTION_POINTERS) -> i32 {
    if !CRASHED.swap(true, Ordering::SeqCst) {
        // SAFETY: Windows passes the record of the exception.
        let record = unsafe { &*(*pointers).ExceptionRecord };
        let count = (record.NumberParameters as usize).min(record.ExceptionInformation.len());
        let exception = Exception {
            code: record.ExceptionCode,
            info: &record.ExceptionInformation[..count],
        };
        let mut name = NameBuffer::default();
        let mut path = [0; 260];
        log::write_unlocked(
            Level::Error,
            format_args!(
                "crash in the {} thread, the engine is stopping: {exception} at {}",
                Wide {
                    text: thread_name(&mut name),
                    or: "unnamed",
                },
                Place::of(record.ExceptionAddress, &mut path),
            ),
        );
        hook::give_up();
        tray::remove_now();
    }
    match PREVIOUS.get().copied().flatten() {
        // SAFETY: the filter Windows had before ours, with the same exception.
        Some(previous) => unsafe { previous(pointers) },
        // Windows goes on with its usual handling: Windows Error Reporting,
        // then the process ends with the exception code.
        None => EXCEPTION_CONTINUE_SEARCH,
    }
}

/// The code Microsoft's C++ compiler uses for C++ exceptions ("msc").
const CPP_EXCEPTION: NTSTATUS = NTSTATUS(0xE06D_7363_u32 as i32);

/// An exception code, with a name and the details for the common ones.
struct Exception<'a> {
    code: NTSTATUS,
    /// The parameters of the exception, whose meaning depends on the code.
    info: &'a [usize],
}

impl fmt::Display for Exception<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exception 0x{:08X}", self.code.0 as u32)?;
        match self.code {
            EXCEPTION_ACCESS_VIOLATION => {
                f.write_str(" (access violation")?;
                // What the code tried to do, and the address it tried it at.
                if let [kind, address, ..] = self.info {
                    let kind = match kind {
                        0 => "reading",
                        1 => "writing",
                        8 => "executing",
                        _ => "at",
                    };
                    write!(f, " {kind} 0x{address:X}")?;
                }
                f.write_char(')')
            }
            EXCEPTION_STACK_OVERFLOW => f.write_str(" (stack overflow)"),
            EXCEPTION_ILLEGAL_INSTRUCTION => f.write_str(" (illegal instruction)"),
            EXCEPTION_INT_DIVIDE_BY_ZERO => f.write_str(" (division by zero)"),
            CPP_EXCEPTION => f.write_str(" (C++ exception)"),
            _ => Ok(()),
        }
    }
}

/// Where an address is: the file of a module and the offset into it, the way
/// a debugger or the module's symbols find it again.
enum Place<'a> {
    Module {
        file: &'a [u16],
        offset: usize,
    },
    /// No module has the address: the code jumped somewhere it should not.
    Unknown(usize),
}

impl<'a> Place<'a> {
    /// `path` is room for the module's path.
    fn of(address: *const c_void, path: &'a mut [u16; 260]) -> Self {
        let mut module = HMODULE::default();
        // SAFETY: with FROM_ADDRESS, the "name" is any address in the module.
        // UNCHANGED_REFCOUNT makes it a plain lookup, with nothing to free.
        let found = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(address.cast()),
                &mut module,
            )
        };
        if found.is_err() {
            return Place::Unknown(address as usize);
        }
        // SAFETY: writes at most `path.len()` characters; 0 on failure.
        let len = unsafe { GetModuleFileNameW(Some(module), path) } as usize;
        Place::Module {
            file: file_name(&path[..len.min(path.len())]),
            offset: address as usize - module.0 as usize,
        }
    }
}

impl fmt::Display for Place<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Place::Module { file, offset } => {
                let file = Wide {
                    text: file,
                    or: "unknown module",
                };
                write!(f, "{file}+0x{offset:X}")
            }
            Place::Unknown(address) => write!(f, "0x{address:X}, outside any module"),
        }
    }
}

/// The part of a path after the last backslash.
fn file_name(path: &[u16]) -> &[u16] {
    path.rsplit(|&c| c == u16::from(b'\\'))
        .next()
        .unwrap_or(path)
}

/// Room for the thread name as Windows returns it: the header of the string,
/// then the text, up to 64 characters. `u64` keeps the header aligned.
type NameBuffer = [u64; 18];

/// The current thread's name, which Rust also gives Windows for the threads it
/// names. `GetThreadDescription` returns a copy on the heap; this is the call
/// it makes inside, into `buffer`. Empty without a name or when it is longer
/// than `buffer` holds.
fn thread_name(buffer: &mut NameBuffer) -> &[u16] {
    let size = size_of::<NameBuffer>();
    // SAFETY: Windows writes at most `size` bytes into `buffer`.
    let status = unsafe {
        NtQueryInformationThread(
            GetCurrentThread(),
            ThreadNameInformation,
            buffer.as_mut_ptr().cast(),
            size as u32,
            ptr::null_mut(),
        )
    };
    if status.is_err() {
        return &[];
    }
    let base = buffer.as_mut_ptr().cast::<u8>();
    // SAFETY: on success, the buffer starts with the header of the string.
    let header = unsafe { &*base.cast::<UNICODE_STRING>() };
    let len = usize::from(header.Length) / 2;
    // The text follows the header in the same buffer; the checks keep it so.
    let offset = (header.Buffer.0 as usize).wrapping_sub(base as usize);
    if len == 0 || !offset.is_multiple_of(2) || offset.saturating_add(len * 2) > size {
        return &[];
    }
    // SAFETY: the text is inside `buffer`, aligned, and Windows wrote it.
    unsafe { slice::from_raw_parts(base.add(offset).cast::<u16>(), len) }
}

/// UTF-16 text from Windows, written out without a `String`.
struct Wide<'a> {
    text: &'a [u16],
    /// What to write when `text` is empty.
    or: &'static str,
}

impl fmt::Display for Wide<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.text.is_empty() {
            return f.write_str(self.or);
        }
        for c in char::decode_utf16(self.text.iter().copied()) {
            f.write_char(c.unwrap_or(char::REPLACEMENT_CHARACTER))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Stdio};

    use windows::Win32::System::Diagnostics::Debug::{
        RaiseException, SEM_NOGPFAULTERRORBOX, SetErrorMode,
    };

    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn names_an_access_violation_and_what_it_tried() {
        let read = Exception {
            code: EXCEPTION_ACCESS_VIOLATION,
            info: &[0, 0x18],
        };
        assert_eq!(
            read.to_string(),
            "exception 0xC0000005 (access violation reading 0x18)"
        );
        let execute = Exception {
            code: EXCEPTION_ACCESS_VIOLATION,
            info: &[8, 0x7FF6_1234_0000],
        };
        assert_eq!(
            execute.to_string(),
            "exception 0xC0000005 (access violation executing 0x7FF612340000)"
        );
        let no_details = Exception {
            code: EXCEPTION_ACCESS_VIOLATION,
            info: &[],
        };
        assert_eq!(
            no_details.to_string(),
            "exception 0xC0000005 (access violation)"
        );
    }

    #[test]
    fn other_codes_show_their_number() {
        let overflow = Exception {
            code: EXCEPTION_STACK_OVERFLOW,
            info: &[],
        };
        assert_eq!(
            overflow.to_string(),
            "exception 0xC00000FD (stack overflow)"
        );
        let unknown = Exception {
            code: NTSTATUS(0xC000_0374_u32 as i32),
            info: &[1, 2],
        };
        assert_eq!(unknown.to_string(), "exception 0xC0000374");
    }

    #[test]
    fn a_place_is_the_module_file_and_the_offset() {
        let path = wide(r"C:\WINDOWS\SYSTEM32\d2d1.dll");
        let place = Place::Module {
            file: file_name(&path),
            offset: 0x19_A186,
        };
        assert_eq!(place.to_string(), "d2d1.dll+0x19A186");
        let lost = Place::Module {
            file: file_name(&[]),
            offset: 0x10,
        };
        assert_eq!(lost.to_string(), "unknown module+0x10");
        assert_eq!(
            Place::Unknown(0xDEAD_0000).to_string(),
            "0xDEAD0000, outside any module"
        );
    }

    #[test]
    fn finds_the_module_of_its_own_code() {
        let mut path = [0; 260];
        let here = finds_the_module_of_its_own_code as *const c_void;
        let place = Place::of(here, &mut path).to_string().to_lowercase();
        // The test executable is named after the crate, plus a hash.
        assert!(place.starts_with("thumbring-"), "{place}");
        assert!(place.contains(".exe+0x"), "{place}");
    }

    #[test]
    fn reads_the_name_rust_gave_the_thread() {
        let name = thread::Builder::new()
            .name("filter-name".into())
            .spawn(|| {
                let mut buffer = NameBuffer::default();
                String::from_utf16(thread_name(&mut buffer)).unwrap()
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(name, "filter-name");
    }

    /// Set only in the process that `a_native_crash_is_logged` starts: the
    /// log file to write.
    const CHILD_LOG: &str = "THUMBRING_TEST_CRASH_LOG";

    /// Starts this same test in a new process, which crashes on purpose, and
    /// reads the log it leaves.
    #[test]
    fn a_native_crash_is_logged() {
        if let Some(log_file) = std::env::var_os(CHILD_LOG) {
            crash(Path::new(&log_file));
        }
        let log_file =
            std::env::temp_dir().join(format!("thumbring-crash-test-{}.log", std::process::id()));
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["crash::tests::a_native_crash_is_logged", "--exact"])
            .env(CHILD_LOG, &log_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        let log = fs::read_to_string(&log_file).unwrap_or_default();
        let _ = fs::remove_file(&log_file);

        // Windows still ended the process as usual, with the exception code.
        assert_eq!(status.code(), Some(EXCEPTION_ACCESS_VIOLATION.0), "{log}");
        let line = "error crash in the crash-test thread, the engine is stopping: \
                    exception 0xC0000005 (access violation writing 0x18) at ";
        let (_, place) = log
            .split_once(line)
            .unwrap_or_else(|| panic!("no crash line in the log: {log:?}"));
        // `RaiseException` lives in KernelBase.
        assert!(
            place.to_lowercase().starts_with("kernelbase.dll+0x"),
            "{log}"
        );
    }

    /// The process that `a_native_crash_is_logged` starts: an access
    /// violation on a named thread, which no code handles.
    fn crash(log_file: &Path) -> ! {
        // SAFETY: only changes this process's error mode. Without the dialog
        // and the report of Windows Error Reporting, the process just ends.
        unsafe { SetErrorMode(SEM_NOGPFAULTERRORBOX) };
        log::init(log_file).unwrap();
        install_exception_filter();
        let _ = thread::Builder::new()
            .name("crash-test".into())
            // SAFETY: raises an exception that only the filter sees. The
            // parameters say "writing at 0x18".
            .spawn(|| unsafe {
                RaiseException(EXCEPTION_ACCESS_VIOLATION.0 as u32, 0, Some(&[1, 0x18]));
            })
            .unwrap()
            .join();
        unreachable!("the crash ends the process");
    }
}
