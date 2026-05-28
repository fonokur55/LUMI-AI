use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    pub available_ram_mb: u64,
    pub total_ram_mb: u64,
    pub cpu_percent: f32,
    pub cpu_cores: usize,
}

pub struct HardwareMonitor {
    sys: System,
}

impl Default for HardwareMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::everything())
                .with_cpu(CpuRefreshKind::everything()),
        );
        Self { sys }
    }

    pub fn snapshot(&mut self) -> HardwareSnapshot {
        // FONTOS: korábban itt egy `std::thread::sleep(200ms)` volt két
        // `refresh_cpu_usage` között, hogy egy delta CPU-mérés legyen.
        // Csakhogy ez a függvény egy `std::sync::Mutex` mögött futott
        // (`state.hardware.lock()`), és a Tauri async kontextusában a
        // blokkoló sleep beragasztotta az egész parancsláncot - emiatt
        // előfordult, hogy az `akasha_status` percekig nem tért vissza,
        // és a chat-küldés sosem érte el az `akasha_chat`-et.
        // A delta-mérés helyett a sysinfo halmozott CPU-értékét használjuk:
        // RAM értékek mindig pontosak, a CPU százalék pedig elég közelítés
        // a throttling-döntésekhez.
        self.sys.refresh_memory();
        self.sys.refresh_cpu_usage();

        let total = self.sys.total_memory();
        let available = self.sys.available_memory();
        let cpu_percent = self.sys.global_cpu_usage();

        HardwareSnapshot {
            available_ram_mb: available / 1024 / 1024,
            total_ram_mb: total / 1024 / 1024,
            cpu_percent,
            cpu_cores: self.sys.cpus().len(),
        }
    }

    pub fn is_critical(&mut self, ram_critical_mb: u64, cpu_critical: f32) -> bool {
        let s = self.snapshot();
        s.available_ram_mb < ram_critical_mb || s.cpu_percent >= cpu_critical
    }

    pub fn is_warning(&mut self, ram_warning_mb: u64, cpu_critical: f32) -> bool {
        let s = self.snapshot();
        s.available_ram_mb < ram_warning_mb || s.cpu_percent >= cpu_critical * 0.85
    }
}
