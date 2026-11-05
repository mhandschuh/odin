use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub notifications: bool,
    /// The connect offer was answered (UC-05): it isn't shown again.
    pub offer_answered: bool,
    /// The status bar with Claude's limits (UC-06).
    pub limits: bool,
    /// The weekly limits per model, from Anthropic (UC-06.3): the only network Odin uses, off until turned on.
    pub model_limits: bool,
    /// The tree's width, as dragged (UC-01.8).
    pub tree_width: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { notifications: true, offer_answered: false, limits: true, model_limits: false, tree_width: 320.0 }
    }
}

impl Prefs {
    pub fn load(file: &Path) -> Prefs {
        std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, file: &Path) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, serde_json::to_string_pretty(self).unwrap_or_default());
    }
}
