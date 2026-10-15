//! Start with Windows as administrator: a Task Scheduler task that starts
//! Thumbring with administrator rights when its user signs in, with no UAC
//! prompt.
//!
//! Only a program with administrator rights may create a task that runs with
//! them, so creating it goes through `schtasks.exe` and one UAC prompt.
//! Reading the task needs no rights, and the task lets its own user delete it,
//! so turning the option off asks nothing (an older task without that
//! permission falls back to the UAC prompt).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{E_ACCESSDENIED, ERROR_FILE_NOT_FOUND, HWND};
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::System::TaskScheduler::{ITaskFolder, ITaskService, TaskScheduler};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::System::WindowsProgramming::GetUserNameW;
use windows::core::{BSTR, PWSTR};

use crate::elevation::{self, Failure};
use crate::log::warning;

/// What [`definition`] needs.
pub struct Definition<'a> {
    /// The user the task belongs to, as a security identifier (`S-1-5-...`).
    pub user_sid: &'a str,
    /// Full path of `thumbring.exe`.
    pub exe: &'a str,
}

/// The task, in the XML format of the Task Scheduler.
///
/// - Starts when this user signs in, with the highest rights the user has.
/// - Priority 4 is the normal one (the default, 7, is below normal and would
///   slow the ring down).
/// - No time limit (`PT0S`), no stop on battery, and no second copy while one
///   is running.
/// - Its user may read and delete it (`0x130089`: read plus delete), but not
///   change it: whoever changes the task decides what runs as administrator.
pub fn definition(task: &Definition) -> String {
    let sid = escape(task.user_sid);
    let exe = escape(task.exe);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Thumbring</Author>
    <Description>Starts Thumbring with administrator rights when you sign in, so its gestures and ring also work in windows that run as administrator. Turn it off in the Thumbring tray menu.</Description>
    <SecurityDescriptor>D:(A;;FA;;;BA)(A;;FA;;;SY)(A;;0x130089;;;{sid})</SecurityDescriptor>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Task name, one per user of the computer, in the root folder of the Task
/// Scheduler. The installer deletes it by the same name.
pub fn name(user: &str) -> String {
    format!("Thumbring ({user})")
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// UTF-16 with a byte order mark, the encoding `schtasks.exe` reads.
fn utf16_file(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

/// Whether the task exists and is enabled. Disabling it in the Task
/// Scheduler counts as off.
pub fn is_enabled() -> bool {
    let name = name(&user_name());
    let result = with_root_folder(|folder| {
        // SAFETY: a COM call with a string that lives during it.
        let task = unsafe { folder.GetTask(&BSTR::from(name.as_str())) }?;
        // SAFETY: a COM call on a valid task.
        Ok(unsafe { task.Enabled() }?.as_bool())
    });
    match result {
        Ok(enabled) => enabled,
        Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => false,
        Err(e) => {
            warning!("could not read the task {name}: {e}");
            false
        }
    }
}

/// Creates (or replaces) the task: one UAC prompt, unless Thumbring has
/// administrator rights already.
pub fn create(exe: &Path, owner: HWND) -> Result<(), Failure> {
    let name = name(&user_name());
    let user_sid = elevation::user_sid().map_err(Failure::Error)?;
    let xml = definition(&Definition {
        user_sid: &user_sid,
        exe: &exe.to_string_lossy(),
    });
    let file = TaskFile::write(&utf16_file(&xml))
        .map_err(|e| Failure::Error(format!("could not write the task file: {e}")))?;
    let args = format!("/create /tn \"{name}\" /xml \"{}\" /f", file.path.display());
    schtasks(&args, owner)
}

/// Deletes the task, without asking if its permissions allow it.
pub fn delete(owner: HWND) -> Result<(), Failure> {
    let name = name(&user_name());
    let result = with_root_folder(|folder| {
        // SAFETY: a COM call with a string that lives during it.
        unsafe { folder.DeleteTask(&BSTR::from(name.as_str()), 0) }
    });
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() => Ok(()),
        Err(e) if e.code() == E_ACCESSDENIED => {
            schtasks(&format!("/delete /tn \"{name}\" /f"), owner)
        }
        Err(e) => Err(Failure::Error(e.to_string())),
    }
}

/// Runs `schtasks.exe` with administrator rights.
fn schtasks(args: &str, owner: HWND) -> Result<(), Failure> {
    let program = system_folder().join("schtasks.exe");
    match elevation::run_as_administrator(&program, args, owner)? {
        0 => Ok(()),
        code => Err(Failure::Error(format!(
            "schtasks.exe {args} ended with code {code}"
        ))),
    }
}

/// Connects to the Task Scheduler and runs `f` on its root folder.
fn with_root_folder<R>(
    f: impl FnOnce(&ITaskFolder) -> windows::core::Result<R>,
) -> windows::core::Result<R> {
    // SAFETY: COM is initialized and uninitialized on this same thread, and
    // the objects it creates are released before CoUninitialize.
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let result = (|| {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let local = VARIANT::default();
            service.Connect(&local, &local, &local, &local)?;
            let folder = service.GetFolder(&BSTR::from("\\"))?;
            f(&folder)
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        result
    }
}

/// The name of the user running Thumbring, as the installer sees it too.
fn user_name() -> String {
    let mut buffer = [0u16; 257];
    let mut size = buffer.len() as u32;
    // SAFETY: `size` is the buffer's length; on success it counts the NUL.
    match unsafe { GetUserNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size) } {
        Ok(()) if size > 0 => String::from_utf16_lossy(&buffer[..size as usize - 1]),
        _ => std::env::var("USERNAME").unwrap_or_default(),
    }
}

/// `C:\Windows\System32`, so `schtasks.exe` never comes from another folder.
fn system_folder() -> PathBuf {
    let mut buffer = [0u16; 260];
    // SAFETY: writes at most the buffer's length and returns how much it wrote.
    let length = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    if length == 0 || length >= buffer.len() {
        return PathBuf::from(r"C:\Windows\System32");
    }
    PathBuf::from(String::from_utf16_lossy(&buffer[..length]))
}

/// The task definition in a temporary file, for `schtasks.exe`. While it
/// exists, the file stays open in a way that lets others read it but not
/// change it, so nothing can swap the definition before `schtasks.exe`, with
/// administrator rights, reads it. Deleted when dropped.
struct TaskFile {
    path: PathBuf,
    lock: Option<File>,
}

impl TaskFile {
    fn write(bytes: &[u8]) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("thumbring-task-{}.xml", std::process::id()));
        // A leftover from a run that stopped halfway.
        let _ = fs::remove_file(&path);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(bytes)?;
        let mut lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&path)?;
        // Between the write and the lock, another program could have changed it.
        let mut written = Vec::new();
        lock.read_to_end(&mut written)?;
        if written != bytes {
            return Err(std::io::Error::other(
                "the file changed after it was written",
            ));
        }
        Ok(Self {
            path,
            lock: Some(lock),
        })
    }
}

impl Drop for TaskFile {
    fn drop(&mut self) {
        // Closed first: the file cannot be deleted while it is locked.
        self.lock.take();
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "S-1-5-21-1-2-3-1001";

    fn sample() -> String {
        definition(&Definition {
            user_sid: SID,
            exe: r"C:\Users\Ana & Bia\AppData\Local\Programs\Thumbring\thumbring.exe",
        })
    }

    #[test]
    fn starts_at_sign_in_of_its_user_with_the_highest_rights() {
        let xml = sample();
        assert!(xml.contains(&format!(
            "<LogonTrigger>\n      <Enabled>true</Enabled>\n      <UserId>{SID}</UserId>"
        )));
        assert!(xml.contains(&format!(
            "<Principal id=\"Author\">\n      <UserId>{SID}</UserId>"
        )));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    }

    #[test]
    fn runs_at_normal_priority_for_as_long_as_needed() {
        let xml = sample();
        assert!(xml.contains("<Priority>4</Priority>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
    }

    #[test]
    fn its_user_may_read_and_delete_it_but_not_change_it() {
        let xml = sample();
        let start = xml.find("<SecurityDescriptor>").unwrap() + "<SecurityDescriptor>".len();
        let end = xml.find("</SecurityDescriptor>").unwrap();
        let user_aces: Vec<_> = xml[start..end]
            .split('(')
            .filter(|ace| ace.ends_with(&format!(";{SID})")))
            .collect();
        // FILE_GENERIC_READ (0x120089) plus DELETE (0x10000), nothing else.
        assert_eq!(user_aces, [format!("A;;0x130089;;;{SID})")]);
    }

    #[test]
    fn the_program_path_is_escaped() {
        assert!(sample().contains(
            r"<Command>C:\Users\Ana &amp; Bia\AppData\Local\Programs\Thumbring\thumbring.exe</Command>"
        ));
        assert_eq!(
            escape(r#"<a> & "b" 'c'"#),
            "&lt;a&gt; &amp; &quot;b&quot; &apos;c&apos;"
        );
    }

    #[test]
    fn the_file_is_utf16_with_a_byte_order_mark() {
        let bytes = utf16_file("<é>");
        assert_eq!(bytes, [0xFF, 0xFE, b'<', 0, 0xE9, 0, b'>', 0]);
        assert!(sample().starts_with("<?xml version=\"1.0\" encoding=\"UTF-16\"?>"));
    }

    #[test]
    fn each_user_has_a_task() {
        assert_eq!(name("ana"), "Thumbring (ana)");
    }
}
