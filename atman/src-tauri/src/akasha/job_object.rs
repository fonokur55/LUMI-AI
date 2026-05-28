//! Windows Job Object wrapper - biztosítja, hogy a spawnolt llama-server
//! child processek MINDIG meghaljanak amikor az atman.exe leáll, akár normál
//! módon, akár crash-szel, akár force-killel.
//!
//! Mechanika (Windows-specifikus):
//! 1. CreateJobObject - létrehozunk egy job-ot
//! 2. SetInformationJobObject (JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE) - beállítjuk
//!    hogy a job lezárása minden processzt megöljön a job-on belül
//! 3. AssignProcessToJobObject - minden spawnolt llama-server-t hozzáadunk
//! 4. Amíg az atman.exe él, a job HANDLE-t fogja → a job nem zárul be
//! 5. Amikor az atman.exe leáll (BÁRHOGY), a Windows automatikusan
//!    bezárja az összes handle-jét → a job lezárul → child-ek meghalnak
//!
//! Még a child PROCESS-EI is meghalnak - Windows 8+-on a job assignment
//! öröklődik a leszármazott processzekre, tehát a llama-server router
//! ÉS a tényleges modell-loader child egyaránt el lesz takarítva.
//!
//! Non-Windows platformokon (macOS, Linux): no-op stub - ott a process-group
//! kezelés (`SIGTERM` minden child-nek) általában jól működik.

#[cfg(windows)]
pub use windows_impl::JobObject;

#[cfg(not(windows))]
pub use stub_impl::JobObject;

#[cfg(windows)]
mod windows_impl {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    pub struct JobObject {
        handle: HANDLE,
    }

    impl JobObject {
        pub fn new() -> Result<Self, String> {
            unsafe {
                let handle = CreateJobObjectW(None, windows::core::PCWSTR::null())
                    .map_err(|e| format!("CreateJobObject sikertelen: {e}"))?;

                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
                    std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags =
                    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()
                        as u32,
                )
                .map_err(|e| {
                    let _ = CloseHandle(handle);
                    format!("SetInformationJobObject sikertelen: {e}")
                })?;

                Ok(JobObject { handle })
            }
        }

        /// Hozzáadja a megadott process-t a job-hoz. Innentől fogva minden
        /// child process-e is automatikusan a job tagja lesz.
        pub fn assign(&self, pid: u32) -> Result<(), String> {
            unsafe {
                let process = OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                    false,
                    pid,
                )
                .map_err(|e| format!("OpenProcess sikertelen (pid={pid}): {e}"))?;

                let res = AssignProcessToJobObject(self.handle, process)
                    .map_err(|e| format!("AssignProcessToJobObject sikertelen: {e}"));
                let _ = CloseHandle(process);
                res
            }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // CloseHandle → ha JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE be van állítva,
            // a Windows kill-eli az összes assigned process-t. Ez a "safety net"
            // - még akkor is fut, ha az atman.exe a process-tábláról törlődik,
            // mert az OS kezel mindent.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    // A HANDLE Send + Sync, mert csak read-only handle-műveleteket csinálunk
    // több thread-ről (Mutex védi az Option<JobObject>-et a hívó oldalon).
    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}
}

#[cfg(not(windows))]
mod stub_impl {
    /// macOS/Linux no-op. Itt a parent halálával a child-ek általában
    /// rendben záródnak (SIGTERM, process-group). Ha a jövőben kell, itt
    /// lehet pl. setpgid + killpg pattern.
    pub struct JobObject;

    impl JobObject {
        pub fn new() -> Result<Self, String> {
            Ok(JobObject)
        }
        pub fn assign(&self, _pid: u32) -> Result<(), String> {
            Ok(())
        }
    }
}
