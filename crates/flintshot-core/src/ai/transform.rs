//! Saved AI transforms — the user's CRUD library of named prompts.
//!
//! File format (`<config_dir>/flintshot/ai-transforms.toml`): an
//! array of `[[transform]]` tables in kebab-case, e.g.
//!
//! ```toml
//! [[transform]]
//! id = "0b6ce6ae-6e5b-4c04-93c6-a8b6e0d3c10d"
//! name = "Extract text"
//! prompt = "Extract the text inside this image"
//! ```
//!
//! This is CRUD user data, deliberately **not** part of
//! `config.toml` (which holds only the `[ai]` provider choice). The
//! load path is session-store tolerant: a missing or malformed file
//! yields an empty store with a `tracing::warn!` — never an `Err` —
//! so a bad file can't block the editor. Saves are atomic
//! (temp file + rename) so a crash mid-write can't corrupt the list.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A saved user transform, persisted in `ai-transforms.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiTransform {
    /// Stable id (uuid v4) — rename-safe for the dialog list.
    pub id: String,
    /// Short human name ("Extract text").
    pub name: String,
    /// The instruction sent with the image.
    pub prompt: String,
    /// Marks the non-deletable built-in starters (duplicable into
    /// the user store, never stored there). Serde-defaulted and
    /// skipped when false so user files stay clean.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub builtin: bool,
}

/// File envelope: `[[transform]]` tables under the hood.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TransformFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    transform: Vec<AiTransform>,
}

/// The user's saved transforms, in display order.
///
/// In-memory only until [`TransformStore::save`] — CRUD methods
/// mutate and the caller decides when to persist (the config page
/// saves per edit; the dialog only reads).
#[derive(Debug, Default, Clone)]
pub struct TransformStore {
    transforms: Vec<AiTransform>,
}

/// Resolve `<config_dir>/flintshot/ai-transforms.toml`, or `None`
/// where `dirs::config_dir()` has no answer (same fallback shape as
/// [`crate::config::config_path`]).
#[must_use]
pub fn transform_store_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("flintshot")
            .join("ai-transforms.toml"),
    )
}

impl TransformStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from the platform path; missing or malformed file ⇒
    /// empty store (warn, never fail — session-store stance).
    #[must_use]
    pub fn load() -> Self {
        match transform_store_path() {
            Some(path) => Self::load_from(&path),
            None => Self::new(),
        }
    }

    /// Testable seam: load from an explicit path.
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str::<TransformFile>(&contents) {
                Ok(file) => Self {
                    transforms: file.transform,
                },
                Err(e) => {
                    tracing::warn!(
                        "malformed {}: {e}; starting with no saved transforms",
                        path.display()
                    );
                    Self::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::new(),
            Err(e) => {
                tracing::warn!(
                    "unreadable {}: {e}; starting with no saved transforms",
                    path.display()
                );
                Self::new()
            }
        }
    }

    /// Save to the platform path.
    ///
    /// # Errors
    /// [`crate::FlintshotError::Io`] on write/rename failure, or
    /// [`crate::FlintshotError::Config`] when no platform config dir
    /// exists.
    pub fn save(&self) -> crate::Result<()> {
        match transform_store_path() {
            Some(path) => self.save_to(&path),
            None => Err(crate::FlintshotError::Config {
                path: PathBuf::new(),
                detail: "no platform config_dir available".into(),
            }),
        }
    }

    /// Testable seam: save to an explicit path, atomically (write a
    /// sibling temp file, then rename over the destination).
    ///
    /// # Errors
    /// [`crate::FlintshotError::Io`] on write/rename failure.
    pub fn save_to(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::FlintshotError::Io)?;
        }
        let file = TransformFile {
            transform: self.transforms.clone(),
        };
        let body = toml::to_string_pretty(&file).map_err(|e| crate::FlintshotError::Config {
            path: path.to_path_buf(),
            detail: format!("serialization failure: {e}"),
        })?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body).map_err(crate::FlintshotError::Io)?;
        // Windows' rename-over-existing fails; remove-then-rename is
        // the fallback (the atomic window doesn't exist there anyway).
        if std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp, path).map_err(crate::FlintshotError::Io)?;
        }
        Ok(())
    }

    /// All transforms, in display order.
    #[must_use]
    pub fn all(&self) -> &[AiTransform] {
        &self.transforms
    }

    /// Look up one transform by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&AiTransform> {
        self.transforms.iter().find(|t| t.id == id)
    }

    /// Append a new transform with a fresh uuid v4 id.
    pub fn insert(&mut self, name: impl Into<String>, prompt: impl Into<String>) -> AiTransform {
        let transform = AiTransform {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            prompt: prompt.into(),
            builtin: false,
        };
        self.transforms.push(transform.clone());
        transform
    }

    /// Rewrite `name`/`prompt` of the transform with `id`. Returns
    /// `false` (touching nothing) when the id is unknown.
    pub fn update(&mut self, id: &str, name: impl Into<String>, prompt: impl Into<String>) -> bool {
        let Some(target) = self.transforms.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        target.name = name.into();
        target.prompt = prompt.into();
        true
    }

    /// Remove the transform with `id`. Returns whether it existed.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.transforms.len();
        self.transforms.retain(|t| t.id != id);
        before != self.transforms.len()
    }

    /// Move the transform with `id` by `delta` positions (negative
    /// = up). Clamped so moves past either end are no-ops. Returns
    /// whether anything moved.
    pub fn move_by(&mut self, id: &str, delta: isize) -> bool {
        let Some(index) = self.transforms.iter().position(|t| t.id == id) else {
            return false;
        };
        let last = self.transforms.len().saturating_sub(1);
        let delta_abs = delta.unsigned_abs();
        let target = if delta.is_negative() {
            index.saturating_sub(delta_abs)
        } else {
            (index + delta_abs).min(last)
        };
        if target == index {
            return false;
        }
        // Adjacent moves are swaps; larger deltas still just splice.
        let transform = self.transforms.remove(index);
        self.transforms.insert(target, transform);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn store_path(dir: &Path) -> PathBuf {
        dir.join("flintshot").join("ai-transforms.toml")
    }

    #[test]
    fn crud_round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let path = store_path(dir.path());

        let mut store = TransformStore::load_from(&path);
        assert!(store.all().is_empty(), "missing file loads empty");

        let a = store.insert("Extract text", "Extract the text inside this image");
        let b = store.insert("Summarize", "Summarize this screenshot");
        store.save_to(&path).unwrap();

        let reloaded = TransformStore::load_from(&path);
        assert_eq!(reloaded.all(), store.all(), "same order and content");
        assert_eq!(reloaded.all().len(), 2);
        assert_eq!(reloaded.all()[0].id, a.id);
        assert_eq!(reloaded.all()[1].id, b.id);
        assert_eq!(reloaded.all()[0].name, "Extract text");
        assert_eq!(reloaded.all()[1].prompt, "Summarize this screenshot");
    }

    #[test]
    fn written_file_uses_transform_tables() {
        let dir = tempdir().unwrap();
        let path = store_path(dir.path());
        let mut store = TransformStore::new();
        store.insert("OCR", "read it");
        store.save_to(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("[[transform]]"), "body was:\n{body}");
        assert!(body.contains("name = \"OCR\""));
        assert!(body.contains("prompt = \"read it\""));
        // The builtin flag is serde-defaulted and skipped when false:
        // user files stay clean of the implementation detail.
        assert!(
            !body.contains("builtin"),
            "user store must not carry a builtin key:\n{body}"
        );
    }

    #[test]
    fn update_and_remove_by_id_leave_others_untouched() {
        let mut store = TransformStore::new();
        let a = store.insert("A", "a-prompt");
        let b = store.insert("B", "b-prompt");
        let _c = store.insert("C", "c-prompt");

        assert!(store.update(&b.id, "B2", "b2-prompt"));
        assert_eq!(store.get(&b.id).unwrap().name, "B2");
        assert_eq!(store.get(&a.id).unwrap().prompt, "a-prompt", "A untouched");
        assert!(
            !store.update("no-such-id", "X", "x"),
            "unknown id is a no-op"
        );

        assert!(store.remove(&b.id));
        assert!(store.get(&b.id).is_none());
        assert_eq!(store.all().len(), 2);
        assert_eq!(store.all()[0].id, a.id, "order of survivors intact");
        assert!(!store.remove(&b.id), "second remove is a no-op");
    }

    #[test]
    fn move_swaps_neighbours_and_no_ops_at_ends() {
        let mut store = TransformStore::new();
        let a = store.insert("A", "p");
        let b = store.insert("B", "p");
        let c = store.insert("C", "p");

        assert!(store.move_by(&c.id, -1), "C moves up over B");
        let ids: Vec<&str> = store.all().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, [a.id.as_str(), c.id.as_str(), b.id.as_str()]);

        assert!(!store.move_by(&a.id, -1), "first entry can't move up");
        assert!(!store.move_by(&b.id, 1), "last entry can't move down");
        assert!(!store.move_by("no-such-id", 1), "unknown id is a no-op");
        let ids: Vec<&str> = store.all().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            [a.id.as_str(), c.id.as_str(), b.id.as_str()],
            "ends stable"
        );
    }

    #[test]
    fn malformed_file_loads_empty_without_error() {
        let dir = tempdir().unwrap();
        let path = store_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is [not toml").unwrap();
        let store = TransformStore::load_from(&path);
        assert!(store.all().is_empty());
    }

    #[test]
    fn empty_transform_list_writes_an_empty_file_that_loads_empty() {
        let dir = tempdir().unwrap();
        let path = store_path(dir.path());
        TransformStore::new().save_to(&path).unwrap();
        let store = TransformStore::load_from(&path);
        assert!(store.all().is_empty());
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let dir = tempdir().unwrap();
        let path = store_path(dir.path());
        let mut store = TransformStore::new();
        store.insert("A", "p");
        store.save_to(&path).unwrap();
        assert!(!path.with_extension("toml.tmp").exists());
        assert!(path.exists());
    }
}
