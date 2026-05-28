use super::job_object::JobObject;
use super::types::AkashaSlot;
use crate::portable::config::AkashaConfig;
use crate::portable::paths::{runtime_binary_path, to_short_path};
use std::fs::OpenOptions;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AkashaStatus {
    Stopped,
    Starting,
    Ready,
    Error,
}

pub struct AkashaRuntime {
    child: Mutex<Option<Child>>,
    pub port: Mutex<u16>,
    pub status: Mutex<AkashaStatus>,
    pub last_error: Mutex<Option<String>>,
    pub active_slot: Mutex<Option<AkashaSlot>>,
    pub active_model_id: Mutex<Option<String>>,
    pub models_dir: Mutex<Option<String>>,
    /// A frontend Stop-gombja állítja `true`-ra. A `stream_chat` minden chunk
    /// után figyeli; ha `true`, megszakítja a streamelést és visszaadja az
    /// addig összegyűlt szöveget. A következő `akasha_chat` hívás elején
    /// automatikusan visszaáll `false`-ra.
    pub cancel_flag: Arc<AtomicBool>,
    /// Windows Job Object - minden spawnolt llama-server process ide kerül.
    /// Amikor az atman.exe leáll (bármilyen okból), az OS bezárja ezt a
    /// handle-t, ami megöli az összes child process-t a job-ban. Ez
    /// megakadályozza, hogy a 5+ GB-os modell-betöltött llama-server
    /// "árván maradjon" és RAM-ot foglaljon az app bezárása után.
    job: Mutex<Option<JobObject>>,
}

impl Default for AkashaRuntime {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
            port: Mutex::new(0),
            status: Mutex::new(AkashaStatus::Stopped),
            last_error: Mutex::new(None),
            active_slot: Mutex::new(None),
            active_model_id: Mutex::new(None),
            models_dir: Mutex::new(None),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            job: Mutex::new(None),
        }
    }
}

impl AkashaRuntime {
    /// A frontend `akasha_cancel_generation` parancsa hívja meg.
    pub fn request_cancel(&self) {
        self.cancel_flag.store(true, Ordering::Release);
    }

    /// Új generálás kezdetén nullázzuk.
    pub fn reset_cancel(&self) {
        self.cancel_flag.store(false, Ordering::Release);
    }

    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }
}

impl AkashaRuntime {
    pub fn get_status(&self) -> AkashaStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(AkashaStatus::Error)
    }

    pub fn base_url(&self) -> Option<String> {
        let port = *self.port.lock().ok()?;
        if port == 0 {
            return None;
        }
        Some(format!("http://127.0.0.1:{port}"))
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child
            .lock()
            .ok()
            .and_then(|c| c.as_ref().map(|ch| ch.id()))
    }

    pub fn set_active(&self, slot: AkashaSlot, model_id: &str) {
        if let Ok(mut s) = self.active_slot.lock() {
            *s = Some(slot);
        }
        if let Ok(mut m) = self.active_model_id.lock() {
            *m = Some(model_id.to_string());
        }
    }

    /// Reseteli az aktív slot/model jelzéseket - az auto-unload után hívjuk,
    /// hogy a UI tudja: jelenleg semmi sincs RAM-ban.
    pub fn clear_active(&self) {
        if let Ok(mut s) = self.active_slot.lock() {
            *s = None;
        }
        if let Ok(mut m) = self.active_model_id.lock() {
            *m = None;
        }
    }

    pub fn stop(&self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        if let Ok(mut s) = self.status.lock() {
            *s = AkashaStatus::Stopped;
        }
        if let Ok(mut p) = self.port.lock() {
            *p = 0;
        }
        if let Ok(mut slot) = self.active_slot.lock() {
            *slot = None;
        }
        if let Ok(mut mid) = self.active_model_id.lock() {
            *mid = None;
        }
    }

    pub async fn start_router(
        &self,
        launch_root: &Path,
        cfg: &AkashaConfig,
        runtime_path: &str,
    ) -> Result<u16, String> {
        self.stop();

        if let Ok(mut s) = self.status.lock() {
            *s = AkashaStatus::Starting;
        }

        let binary = if Path::new(runtime_path).exists() {
            Path::new(runtime_path).to_path_buf()
        } else {
            runtime_binary_path(launch_root)?
        };

        if !binary.exists() {
            let msg = format!(
                "llama-server nem található: {}. Futtasd: scripts/fetch-llama.ps1",
                binary.display()
            );
            self.set_error(&msg);
            return Err(msg);
        }

        let models_dir = cfg.models_dir_path(launch_root);
        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;
        }

        // Windows-on a llama-server child process-ek `fopen` API-val nyitják meg a GGUF-ot,
        // ami nem támogatja az UTF-8 path-okat. A 8.3 short path-szá konverzió kerüli ezt
        // (ékezetes / szóközös mappa nevek miatt: pl. "Áron mappája").
        let models_dir_arg = to_short_path(&models_dir).unwrap_or_else(|| models_dir.clone());

        if let Ok(mut d) = self.models_dir.lock() {
            *d = Some(models_dir.display().to_string());
        }

        let port = pick_port()?;
        let threads = if cfg.n_threads == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(4)
        } else {
            cfg.n_threads
        };

        // llama-server stderr/stdout egy log fájlba - debughoz / felhasználói diagnosztikához.
        let logs_dir = launch_root.join("data").join("logs");
        let _ = std::fs::create_dir_all(&logs_dir);
        let log_path = logs_dir.join("llama-server.log");
        let (stdout_target, stderr_target) = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            Ok(file) => {
                let clone = file.try_clone().unwrap_or_else(|_| {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&log_path)
                        .expect("nyitható log")
                });
                (Stdio::from(file), Stdio::from(clone))
            }
            Err(_) => (Stdio::null(), Stdio::null()),
        };

        let mut cmd = Command::new(&binary);
        cmd.arg("--models-dir")
            .arg(&models_dir_arg)
            .arg("--models-max")
            .arg(cfg.models_max.to_string())
            .arg("--host")
            .arg(&cfg.host)
            .arg("--port")
            .arg(port.to_string())
            .arg("-t")
            .arg(threads.to_string())
            .arg("-c")
            .arg(cfg.n_ctx.to_string())
            .stdout(stdout_target)
            .stderr(stderr_target);

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        // #region agent log
        crate::debug_log::dlog(
            "server.rs:start_router",
            "H7",
            "spawning llama-server",
            serde_json::json!({
                "binary": binary.display().to_string(),
                "binary_exists": binary.exists(),
                "models_dir_original": models_dir.display().to_string(),
                "models_dir_arg_short": models_dir_arg.display().to_string(),
                "models_dir_exists": models_dir.exists(),
                "models_max": cfg.models_max,
                "host": cfg.host,
                "port": port,
                "threads": threads,
                "n_ctx": cfg.n_ctx,
            }),
        );
        // #endregion

        let child = cmd.spawn().map_err(|e| {
            let msg = format!("llama-server indítás sikertelen: {e}");
            self.set_error(&msg);
            // #region agent log
            crate::debug_log::dlog(
                "server.rs:start_router",
                "H1",
                "spawn FAILED",
                serde_json::json!({ "error": e.to_string() }),
            );
            // #endregion
            msg
        })?;

        // === Process-leak védelem: hozzáadjuk a child-et a Job Object-hez ===
        // Ha még nincs job (első spawn), létrehozzuk. Ha már van (újraindítás),
        // ugyanazt használjuk - minden eddigi child már úgyis halott (stop()
        // hívás miatt), de a job lifetime az AkashaRuntime-é.
        let child_pid = child.id();
        {
            let mut job_guard = self.job.lock().map_err(|e| e.to_string())?;
            if job_guard.is_none() {
                match JobObject::new() {
                    Ok(j) => *job_guard = Some(j),
                    Err(e) => {
                        crate::debug_log::dlog(
                            "server.rs:start_router",
                            "H10",
                            "JobObject::new FAILED - process-leak veszély",
                            serde_json::json!({ "error": e }),
                        );
                    }
                }
            }
            if let Some(job) = job_guard.as_ref() {
                if let Err(e) = job.assign(child_pid) {
                    crate::debug_log::dlog(
                        "server.rs:start_router",
                        "H10",
                        "JobObject::assign FAILED",
                        serde_json::json!({ "pid": child_pid, "error": e }),
                    );
                } else {
                    crate::debug_log::dlog(
                        "server.rs:start_router",
                        "H10",
                        "child process job-hoz rendelve",
                        serde_json::json!({ "pid": child_pid }),
                    );
                }
            }
        }

        if let Ok(mut c) = self.child.lock() {
            *c = Some(child);
        }
        if let Ok(mut p) = self.port.lock() {
            *p = port;
        }

        for i in 0..60 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if health_ok(port).await {
                if let Ok(mut s) = self.status.lock() {
                    *s = AkashaStatus::Ready;
                }
                if let Ok(mut e) = self.last_error.lock() {
                    *e = None;
                }
                // #region agent log
                crate::debug_log::dlog(
                    "server.rs:start_router",
                    "H1",
                    "health_ok after spawn",
                    serde_json::json!({ "port": port, "attempt": i }),
                );
                // #endregion
                return Ok(port);
            }
        }

        let msg = "AKASHA szerver nem válaszolt időben.".to_string();
        // #region agent log
        crate::debug_log::dlog(
            "server.rs:start_router",
            "H1",
            "health timeout (15s)",
            serde_json::json!({ "port": port }),
        );
        // #endregion
        self.set_error(&msg);
        self.stop();
        Err(msg)
    }

    fn set_error(&self, msg: &str) {
        if let Ok(mut s) = self.status.lock() {
            *s = AkashaStatus::Error;
        }
        if let Ok(mut e) = self.last_error.lock() {
            *e = Some(msg.to_string());
        }
    }
}

fn pick_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|e| e.to_string())
        .and_then(|l| l.local_addr().map_err(|e| e.to_string()).map(|a| a.port()))
}

async fn health_ok(port: u16) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Bármi HTTP válasz a llama-servertől (akár 4xx is) azt jelzi, hogy él.
    // Router módban a "/models" listázza a presetet, de a verziók eltérhetnek.
    for path in ["/health", "/v1/models", "/models", "/"] {
        let url = format!("http://127.0.0.1:{port}{path}");
        if let Ok(res) = client.get(&url).send().await {
            let code = res.status().as_u16();
            if code < 500 {
                return true;
            }
        }
    }

    // Végső fallback: nyers TCP kapcsolat - ha sikerül, a server fut.
    tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok()
}
