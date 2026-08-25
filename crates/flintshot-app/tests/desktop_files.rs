//! Guard test: the `.desktop` files shipped under `assets/applications/`
//! must (a) parse per the freedesktop.org Desktop Entry Specification's
//! line grammar and (b) stay in sync with the CLI surface — every
//! `Exec=`'s subcommand token must exist in the actual binary's
//! `--help` output.
//!
//! This is the drift guard for the launcher effort: renaming a clap
//! subcommand (or typo-ing an `Exec=`) fails here before it ships a
//! broken launcher entry. A minimal line parser keeps CI hermetic —
//! `desktop-file-validate` is not guaranteed present (spec decision:
//! "Validation via test, not linter dep").

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The `.desktop` files this test guards, relative to
/// `assets/applications/`.
const DESKTOP_FILES: &[&str] = &["flintshot.desktop", "flintshot-autostart.desktop"];

/// Keys required (non-empty) in the main `[Desktop Entry]` group.
/// `Exec` is required by the spec for `Type=Application`; `Icon` is
/// this effort's launcher-entry baseline.
const MAIN_GROUP_REQUIRED_KEYS: &[&str] = &["Type", "Name", "Exec", "Icon"];

/// Keys required in every `[Desktop Action *]` group per the spec's
/// "Desktop Entry Actions" section.
const ACTION_GROUP_REQUIRED_KEYS: &[&str] = &["Name", "Exec"];

/// A `[Group]` header plus its ordered key/value entries.
struct Group {
    header: String,
    entries: Vec<(String, String)>,
}

/// A parsed `.desktop` file.
struct DesktopFile {
    name: String,
    groups: Vec<Group>,
}

impl DesktopFile {
    /// Parse `path` enforcing the entry-file line grammar: comments and
    /// blank lines are skipped, `[header]` opens a group, everything
    /// else must be `key=value` inside a group, and a key may appear at
    /// most once per group. Panics naming the file and line on the
    /// first violation.
    fn parse(path: &Path) -> Self {
        let name = path
            .file_name()
            .unwrap_or_else(|| panic!("path {} has no file name", path.display()))
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));

        let mut groups: Vec<Group> = Vec::new();
        let mut current: Option<Group> = None;
        for (idx, raw) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(inner) = line.strip_prefix('[') {
                let header = inner.strip_suffix(']').unwrap_or_else(|| {
                    panic!("{name}:{line_no}: group header `{line}` lacks a closing `]`")
                });
                assert!(
                    !header.is_empty() && !header.contains(['[', ']']),
                    "{name}:{line_no}: invalid group name `{header}`"
                );
                if let Some(group) = current.take() {
                    groups.push(group);
                }
                current = Some(Group {
                    header: header.to_string(),
                    entries: Vec::new(),
                });
            } else {
                let (key, value) = line.split_once('=').unwrap_or_else(|| {
                    panic!("{name}:{line_no}: line `{line}` is not `key=value`")
                });
                assert!(
                    !key.trim().is_empty(),
                    "{name}:{line_no}: empty key in `{line}`"
                );
                let group = current.as_mut().unwrap_or_else(|| {
                    panic!("{name}:{line_no}: entry `{line}` appears before any group header")
                });
                assert!(
                    !group.entries.iter().any(|(k, _)| k == key),
                    "{name}:{line_no}: duplicate key `{key}` in [{}]",
                    group.header
                );
                group.entries.push((key.to_string(), value.to_string()));
            }
        }
        if let Some(group) = current.take() {
            groups.push(group);
        }
        Self { name, groups }
    }

    /// The group with `header`, if present.
    fn group(&self, header: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.header == header)
    }

    /// Value of `key` in `group`; panics naming file, group, and key
    /// when either is missing.
    fn required_key(&self, group: &str, key: &str) -> &str {
        self.value_of(group, key)
            .unwrap_or_else(|| panic!("{}: [{}] missing required key `{key}`", self.name, group))
    }

    /// Value of `key` in `group`, if present.
    fn value_of(&self, group: &str, key: &str) -> Option<&str> {
        self.group(group)?.value_of(key)
    }

    /// `(group, value)` for every `Exec=` in the file — the main group
    /// and any Desktop Action groups.
    fn exec_values(&self) -> Vec<(&str, &str)> {
        self.groups
            .iter()
            .filter_map(|g| g.value_of("Exec").map(|v| (g.header.as_str(), v)))
            .collect()
    }

    /// Ids of all `[Desktop Action <id>]` groups. Panics on an action
    /// group whose id is empty.
    fn action_ids(&self) -> Vec<&str> {
        self.groups
            .iter()
            .filter_map(|g| {
                g.header.strip_prefix("Desktop Action ").inspect(|id| {
                    assert!(
                        !id.is_empty(),
                        "{}: action group `[{}]` has an empty id",
                        self.name,
                        g.header
                    );
                })
            })
            .collect()
    }
}

impl Group {
    /// Value of `key` in this group, if present.
    fn value_of(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

fn load(name: &str) -> DesktopFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/applications")
        .join(name);
    DesktopFile::parse(&path)
}

/// Subcommand names as listed by `flintshot --help`.
///
/// GOTCHA: clap renders subcommands as indented `name  about` lines
/// under a `Commands:` heading, terminated by a blank line. That layout
/// is stable across clap 4.x, but if a clap upgrade reflows the help
/// text this extraction yields an empty list — the `assert!` below
/// turns that into a named failure instead of a vacuous pass. This
/// function is the single place to update for a new layout.
fn cli_subcommands() -> HashSet<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_flintshot"))
        .arg("--help")
        .output()
        .expect("running `flintshot --help` for the Exec-sync check");
    assert!(
        output.status.success(),
        "`flintshot --help` exited with {}",
        output.status
    );

    let text = String::from_utf8_lossy(&output.stdout);
    let mut names = Vec::new();
    let mut in_commands = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "Commands:" {
            in_commands = true;
        } else if in_commands {
            if trimmed.is_empty() {
                in_commands = false;
            } else if let Some(first) = trimmed.split_whitespace().next() {
                names.push(first.to_string());
            }
        }
    }
    assert!(
        !names.is_empty(),
        "extracted zero subcommands from `--help`; did clap's help layout \
         change? update `cli_subcommands` parsing"
    );
    names.into_iter().collect()
}

#[test]
fn desktop_entry_group_comes_first() {
    for name in DESKTOP_FILES {
        let file = load(name);
        let first = file
            .groups
            .first()
            .unwrap_or_else(|| panic!("{name}: contains no groups"));
        assert_eq!(
            first.header, "Desktop Entry",
            "{name}: first group must be [Desktop Entry]"
        );
    }
}

#[test]
fn required_keys_present_and_nonempty() {
    for name in DESKTOP_FILES {
        let file = load(name);
        for key in MAIN_GROUP_REQUIRED_KEYS {
            let value = file.required_key("Desktop Entry", key);
            assert!(
                !value.trim().is_empty(),
                "{name}: [Desktop Entry] {key} is empty"
            );
        }
        for id in file.action_ids() {
            for key in ACTION_GROUP_REQUIRED_KEYS {
                let value = file.required_key(&format!("Desktop Action {id}"), key);
                assert!(
                    !value.trim().is_empty(),
                    "{name}: [Desktop Action {id}] {key} is empty"
                );
            }
        }
    }
}

#[test]
fn actions_ids_and_sections_match_both_ways() {
    let name = "flintshot.desktop";
    let file = load(name);
    let actions = file.required_key("Desktop Entry", "Actions");

    let declared: Vec<&str> = actions.split(';').filter(|s| !s.is_empty()).collect();
    assert!(
        !declared.is_empty(),
        "{name}: Actions=`{actions}` declares no ids (the launcher entry's \
         quick-capture actions are this effort's core UX)"
    );

    let mut sorted = declared.clone();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(
        before,
        sorted.len(),
        "{name}: Actions=`{actions}` lists duplicate ids"
    );

    let sections = file.action_ids();
    for id in &declared {
        assert!(
            sections.contains(id),
            "{name}: Actions=`{actions}` lists `{id}` but no \
             [Desktop Action {id}] group exists"
        );
    }
    for id in &sections {
        assert!(
            declared.contains(id),
            "{name}: [Desktop Action {id}] group exists but Actions=`{actions}` \
             does not list it"
        );
    }
}

#[test]
fn autostart_variant_has_no_actions() {
    let name = "flintshot-autostart.desktop";
    let file = load(name);
    assert!(
        file.value_of("Desktop Entry", "Actions").is_none(),
        "{name}: autostart variant must not declare Actions (a login session \
         wants the tray, not a capture)"
    );
    let sections = file.action_ids();
    assert!(
        sections.is_empty(),
        "{name}: autostart variant must have no Desktop Action groups, found \
         {sections:?}"
    );
}

#[test]
fn exec_targets_resolve_to_real_cli_subcommands() {
    let subcommands = cli_subcommands();
    for name in DESKTOP_FILES {
        let file = load(name);
        for (group, exec) in file.exec_values() {
            let tokens: Vec<&str> = exec.split_whitespace().collect();
            let binary = tokens
                .first()
                .unwrap_or_else(|| panic!("{name}: [{group}] Exec=`{exec}` is empty"));
            assert_eq!(
                *binary, "flintshot",
                "{name}: [{group}] Exec binary must be `flintshot`, got `{exec}`"
            );
            let sub = tokens.get(1).unwrap_or_else(|| {
                panic!("{name}: [{group}] Exec=`{exec}` has no subcommand token")
            });
            assert!(
                subcommands.contains(*sub),
                "{name}: [{group}] Exec=`{exec}` names subcommand `{sub}`, which \
                 does not exist in `flintshot --help` — drift between the \
                 .desktop files and crates/flintshot-app/src/cli.rs?"
            );
        }
    }
}
