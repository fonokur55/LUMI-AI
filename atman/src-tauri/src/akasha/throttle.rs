use super::hardware::HardwareSnapshot;
use crate::portable::config::AkashaThrottleConfig;
use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThrottleLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrottleStatus {
    pub level: ThrottleLevel,
    pub available_ram_mb: u64,
    pub cpu_percent: f32,
    pub effective_threads: u32,
}

pub struct DynamicThrottle {
    pub level: Mutex<ThrottleLevel>,
    child_pid: Mutex<Option<u32>>,
}

impl Default for DynamicThrottle {
    fn default() -> Self {
        Self {
            level: Mutex::new(ThrottleLevel::Normal),
            child_pid: Mutex::new(None),
        }
    }
}

impl DynamicThrottle {
    pub fn set_child_pid(&self, pid: Option<u32>) {
        if let Ok(mut p) = self.child_pid.lock() {
            *p = pid;
        }
    }

    pub fn evaluate(
        &self,
        hw: &HardwareSnapshot,
        cfg: &AkashaThrottleConfig,
        base_threads: u32,
    ) -> ThrottleStatus {
        let level = if hw.available_ram_mb < cfg.ram_critical_mb
            || hw.cpu_percent >= cfg.cpu_critical_percent
        {
            ThrottleLevel::Critical
        } else if hw.available_ram_mb < cfg.ram_warning_mb
            || hw.cpu_percent >= cfg.cpu_critical_percent * 0.85
        {
            ThrottleLevel::Warning
        } else {
            ThrottleLevel::Normal
        };

        if let Ok(mut l) = self.level.lock() {
            *l = level;
        }

        let cores = hw.cpu_cores.max(2) as u32;
        let auto_threads = if base_threads == 0 {
            cores
        } else {
            base_threads
        };

        let effective_threads = match level {
            ThrottleLevel::Normal => auto_threads,
            ThrottleLevel::Warning => ((auto_threads as f32) * 0.75).max(cfg.min_threads as f32) as u32,
            ThrottleLevel::Critical => cfg.min_threads.max(1),
        };

        self.apply_process_priority(level);

        ThrottleStatus {
            level,
            available_ram_mb: hw.available_ram_mb,
            cpu_percent: hw.cpu_percent,
            effective_threads,
        }
    }

    fn apply_process_priority(&self, level: ThrottleLevel) {
        let pid = self.child_pid.lock().ok().and_then(|p| *p);
        let Some(pid) = pid else { return };

        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Threading::{
                OpenProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, IDLE_PRIORITY_CLASS,
                PROCESS_SET_INFORMATION,
            };
            unsafe {
                if let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
                    let priority = match level {
                        ThrottleLevel::Normal => return,
                        ThrottleLevel::Warning => BELOW_NORMAL_PRIORITY_CLASS,
                        ThrottleLevel::Critical => IDLE_PRIORITY_CLASS,
                    };
                    let _ = SetPriorityClass(handle, priority);
                    let _ = CloseHandle(handle);
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = (level, pid);
        }
    }

    pub fn current_level(&self) -> ThrottleLevel {
        self.level.lock().map(|l| *l).unwrap_or(ThrottleLevel::Normal)
    }
}
