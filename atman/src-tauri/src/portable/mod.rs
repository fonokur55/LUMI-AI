pub mod config;
pub mod paths;

pub use config::{init_config, load_config, migrate_eco_model, save_config, AtmanConfig};
pub use paths::{ensure_portable_layout, get_launch_root, AppPaths};
