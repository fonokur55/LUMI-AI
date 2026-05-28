use crate::akasha::{AkashaRuntime, DynamicThrottle, EtaEstimator, HardwareMonitor};
use crate::chats::ChatStore;
use crate::memory::MemoryStore;
use crate::memory_notes::MemoryNotesStore;
use crate::portable::{AppPaths, AtmanConfig};
use crate::profile::ProfileStore;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub paths: Mutex<AppPaths>,
    pub config: Mutex<AtmanConfig>,
    pub akasha: AkashaRuntime,
    pub memory: Mutex<Option<MemoryStore>>,
    pub memory_notes: Mutex<Option<MemoryNotesStore>>,
    pub profile: Mutex<Option<ProfileStore>>,
    pub chats: Mutex<Option<ChatStore>>,
    pub hardware: Mutex<HardwareMonitor>,
    pub throttle: Arc<DynamicThrottle>,
    pub eta: Arc<EtaEstimator>,
}

impl AppState {
    pub fn new(paths: AppPaths, config: AtmanConfig) -> Result<Self, String> {
        let memory = MemoryStore::open(&paths.vectors_db, &paths.memory_documents)?;
        let memory_notes = MemoryNotesStore::open(&paths.memory_notes_db)?;
        let profile = ProfileStore::open(&paths.profile_db)?;

        // FIRST-RUN logika: ne írjuk felül a DB-ben lévő nevet a config
        // default ("felhasználó") értékkel. A DB üres név = "még nem
        // állította be" - ezt látja a frontend első-indítás-modálban.
        // Akkor írjuk a DB-be a config nevét, ha:
        //   - a DB üres (még nincs név), ÉS
        //   - a config-ban van VALÓDI név (nem üres és nem "felhasználó")
        let db_name = profile.get_display_name().unwrap_or_default();
        if db_name.trim().is_empty() {
            let cfg_name = config.profile.display_name.trim();
            if !cfg_name.is_empty() && cfg_name != "felhasználó" {
                profile.set_display_name(cfg_name)?;
            }
        }

        let chats = ChatStore::open(&paths.chats_db)?;

        Ok(Self {
            paths: Mutex::new(paths),
            config: Mutex::new(config),
            akasha: AkashaRuntime::default(),
            memory: Mutex::new(Some(memory)),
            memory_notes: Mutex::new(Some(memory_notes)),
            profile: Mutex::new(Some(profile)),
            chats: Mutex::new(Some(chats)),
            hardware: Mutex::new(HardwareMonitor::new()),
            throttle: Arc::new(DynamicThrottle::default()),
            eta: Arc::new(EtaEstimator::new()),
        })
    }
}
