use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A folder that the user wants FileGrove to organize.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Uuid,
    pub path: String,
    pub name: String,
    pub mode: FolderMode,
    pub rule_ids: Vec<Uuid>,
    pub watcher_status: WatcherStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub subfolders: SubfolderConfig,
    pub exclude_folders: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl Folder {
    pub(crate) fn recursive_organization_requires_confirmation(
        &self,
        recursive_requested: bool,
    ) -> bool {
        recursive_requested && self.subfolders.enabled && !self.subfolders.first_run_confirmed
    }

    pub(crate) fn reconcile_recursive_confirmation(
        &self,
        requested: &mut SubfolderConfig,
        exclude_folders: &[String],
        exclude_patterns: &[String],
    ) {
        let recursive_scope_changed = self.subfolders.enabled != requested.enabled
            || self.subfolders.destination_mode != requested.destination_mode
            || self.subfolders.max_depth != requested.max_depth
            || self.exclude_folders != exclude_folders
            || self.exclude_patterns != exclude_patterns;

        // `confirm_first_run` is the only command allowed to promote this
        // flag. Configuration payloads may retain a valid confirmation, but
        // can never forge one or carry it across a material scope change.
        requested.first_run_confirmed =
            requested.enabled && !recursive_scope_changed && self.subfolders.first_run_confirmed;
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FolderMode {
    /// Manual one-shot organize; user clicks "Organize now".
    Single,
    /// Real-time: every new file triggers rules immediately.
    Auto,
}

impl FolderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Auto => "auto",
        }
    }
}

impl std::str::FromStr for FolderMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "single" => Ok(Self::Single),
            "auto" => Ok(Self::Auto),
            _ => Err("invalid folder mode"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WatcherStatus {
    Stopped,
    Running,
    Paused,
}

impl WatcherStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubfolderConfig {
    pub enabled: bool,
    pub destination_mode: DestinationMode,
    pub first_run_confirmed: bool,
    pub max_depth: MaxDepth,
}

impl Default for SubfolderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            destination_mode: DestinationMode::Flatten,
            first_run_confirmed: false,
            // Recursive scans are deliberately bounded unless the user
            // explicitly opts into an unlimited traversal.
            max_depth: MaxDepth::Level(5),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DestinationMode {
    /// Move all matched files into the destination root, ignoring subfolder structure.
    Flatten,
    /// Preserve the relative subfolder structure under the destination.
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxDepth {
    Unlimited,
    Level(u32),
}

impl MaxDepth {
    pub fn includes(self, depth: u32) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Level(max) => depth <= max,
        }
    }
}

impl Serialize for MaxDepth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Unlimited => serializer.serialize_str("unlimited"),
            Self::Level(level) => serializer.serialize_u32(*level),
        }
    }
}

impl<'de> Deserialize<'de> for MaxDepth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MaxDepthVisitor;

        impl<'de> de::Visitor<'de> for MaxDepthVisitor {
            type Value = MaxDepth;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("\"unlimited\" or a non-negative integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.eq_ignore_ascii_case("unlimited") {
                    Ok(MaxDepth::Unlimited)
                } else {
                    value
                        .parse::<u32>()
                        .map(MaxDepth::Level)
                        .map_err(|_| E::custom("invalid max depth"))
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u32::try_from(value)
                    .map(MaxDepth::Level)
                    .map_err(|_| E::custom("max depth is too large"))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u32::try_from(value)
                    .map(MaxDepth::Level)
                    .map_err(|_| E::custom("max depth must be non-negative"))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let Some(key) = map.next_key::<String>()? else {
                    return Err(de::Error::custom("invalid max depth"));
                };
                if !key.eq_ignore_ascii_case("level") {
                    return Err(de::Error::custom("invalid max depth"));
                }
                let level = map.next_value::<u32>()?;
                if map.next_key::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("invalid max depth"));
                }
                Ok(MaxDepth::Level(level))
            }
        }

        deserializer.deserialize_any(MaxDepthVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn recursive_folder(confirmed: bool) -> Folder {
        Folder {
            id: Uuid::new_v4(),
            path: "test".into(),
            name: "test".into(),
            mode: FolderMode::Single,
            rule_ids: Vec::new(),
            watcher_status: WatcherStatus::Stopped,
            created_at: Utc::now(),
            subfolders: SubfolderConfig {
                enabled: true,
                destination_mode: DestinationMode::Flatten,
                first_run_confirmed: confirmed,
                max_depth: MaxDepth::Level(5),
            },
            exclude_folders: vec!["private".into()],
            exclude_patterns: vec!["*.tmp".into()],
        }
    }

    #[test]
    fn max_depth_matches_renderer_contract() {
        assert_eq!(
            serde_json::to_string(&MaxDepth::Unlimited).unwrap(),
            "\"unlimited\""
        );
        assert_eq!(serde_json::to_string(&MaxDepth::Level(3)).unwrap(), "3");
        assert_eq!(
            serde_json::from_str::<MaxDepth>("\"unlimited\"").unwrap(),
            MaxDepth::Unlimited
        );
        assert_eq!(
            serde_json::from_str::<MaxDepth>("5").unwrap(),
            MaxDepth::Level(5)
        );
        assert_eq!(
            serde_json::from_str::<MaxDepth>(r#"{"level":5}"#).unwrap(),
            MaxDepth::Level(5)
        );
    }

    #[test]
    fn recursive_execution_requires_backend_confirmation() {
        assert!(recursive_folder(false).recursive_organization_requires_confirmation(true));
        assert!(!recursive_folder(true).recursive_organization_requires_confirmation(true));
        assert!(!recursive_folder(false).recursive_organization_requires_confirmation(false));

        let mut disabled = recursive_folder(false);
        disabled.subfolders.enabled = false;
        assert!(!disabled.recursive_organization_requires_confirmation(true));
    }

    #[test]
    fn unchanged_scope_retains_but_never_forges_confirmation() {
        let current = recursive_folder(true);
        let mut requested = current.subfolders.clone();
        current.reconcile_recursive_confirmation(
            &mut requested,
            &current.exclude_folders,
            &current.exclude_patterns,
        );
        assert!(requested.first_run_confirmed);

        let unconfirmed = recursive_folder(false);
        requested.first_run_confirmed = true;
        unconfirmed.reconcile_recursive_confirmation(
            &mut requested,
            &unconfirmed.exclude_folders,
            &unconfirmed.exclude_patterns,
        );
        assert!(!requested.first_run_confirmed);
    }

    #[test]
    fn material_recursive_scope_changes_reset_confirmation() {
        let current = recursive_folder(true);
        let cases = [
            SubfolderConfig {
                max_depth: MaxDepth::Level(10),
                ..current.subfolders.clone()
            },
            SubfolderConfig {
                destination_mode: DestinationMode::Preserve,
                ..current.subfolders.clone()
            },
            SubfolderConfig {
                enabled: false,
                ..current.subfolders.clone()
            },
        ];

        for mut requested in cases {
            current.reconcile_recursive_confirmation(
                &mut requested,
                &current.exclude_folders,
                &current.exclude_patterns,
            );
            assert!(!requested.first_run_confirmed);
        }

        let mut exclusions_changed = current.subfolders.clone();
        current.reconcile_recursive_confirmation(
            &mut exclusions_changed,
            &["another".into()],
            &current.exclude_patterns,
        );
        assert!(!exclusions_changed.first_run_confirmed);
    }
}
