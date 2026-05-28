use super::types::AkashaSlot;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenStartEvent {
    pub slot: String,
    pub model_id: String,
    pub estimated_total_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenTickEvent {
    pub elapsed_ms: u64,
    pub remaining_ms: u64,
    pub chars_so_far: usize,
}

pub struct EtaEstimator {
    chars_per_sec: Mutex<HashMap<AkashaSlot, f64>>,
}

impl Default for EtaEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl EtaEstimator {
    pub fn new() -> Self {
        let mut defaults = HashMap::new();
        defaults.insert(AkashaSlot::Eco, 45.0);
        defaults.insert(AkashaSlot::Brain, 28.0);
        defaults.insert(AkashaSlot::Creative, 35.0);
        Self {
            chars_per_sec: Mutex::new(defaults),
        }
    }

    pub fn estimate_start(&self, slot: AkashaSlot, model_id: &str, prompt_len: usize) -> GenStartEvent {
        let cps = self.get_cps(slot);
        let expected_chars = (prompt_len as f64 * 1.5).max(200.0);
        let baseline_ms = match slot {
            AkashaSlot::Eco => 8000,
            AkashaSlot::Brain => 45000,
            AkashaSlot::Creative => 25000,
        };
        let estimated_ms = ((expected_chars / cps) * 1000.0) as u64;
        GenStartEvent {
            slot: match slot {
                AkashaSlot::Eco => "eco",
                AkashaSlot::Brain => "brain",
                AkashaSlot::Creative => "creative",
            }
            .to_string(),
            model_id: model_id.to_string(),
            estimated_total_ms: estimated_ms.max(baseline_ms / 2),
        }
    }

    pub fn tick(&self, _slot: AkashaSlot, elapsed_ms: u64, chars_so_far: usize, total_estimate_ms: u64) -> GenTickEvent {
        let remaining = total_estimate_ms.saturating_sub(elapsed_ms);
        let adaptive = if chars_so_far > 0 && elapsed_ms > 0 {
            let cps = chars_so_far as f64 / (elapsed_ms as f64 / 1000.0);
            let expected_total = (chars_so_far as f64 / cps * 1000.0) as u64;
            expected_total.saturating_sub(elapsed_ms)
        } else {
            remaining
        };
        GenTickEvent {
            elapsed_ms,
            remaining_ms: adaptive,
            chars_so_far,
        }
    }

    pub fn record_completion(&self, slot: AkashaSlot, elapsed_ms: u64, chars: usize) {
        if elapsed_ms == 0 || chars == 0 {
            return;
        }
        let cps = chars as f64 / (elapsed_ms as f64 / 1000.0);
        if let Ok(mut map) = self.chars_per_sec.lock() {
            let prev = map.get(&slot).copied().unwrap_or(cps);
            map.insert(slot, prev * 0.7 + cps * 0.3);
        }
    }

    fn get_cps(&self, slot: AkashaSlot) -> f64 {
        self.chars_per_sec
            .lock()
            .ok()
            .and_then(|m| m.get(&slot).copied())
            .unwrap_or(30.0)
    }
}
