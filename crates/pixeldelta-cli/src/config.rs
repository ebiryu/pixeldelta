//! The JSON configuration file `run` and `ci` read, and pattern matching
//! against the relative paths it applies overrides to.

use std::path::{Path, PathBuf};

use pixeldelta_core::Rect;
use serde::Deserialize;

/// Built-in threshold, used when neither the config file nor `--threshold`
/// set one.
const DEFAULT_THRESHOLD: f32 = 0.1;

/// Built-in tolerance ratio, used when neither the config file nor
/// `--tolerance-ratio` set one.
const DEFAULT_TOLERANCE_RATIO: f64 = 0.0;

/// Resolved comparison settings for one entry path.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub threshold: f32,
    pub tolerance_ratio: f64,
    pub ignore_regions: Vec<Rect>,
}

/// A rectangle as it appears in the config file: four `u32` fields, matching
/// [`Rect`] without pulling `serde` into `pixeldelta-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RectDef {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl From<RectDef> for Rect {
    fn from(r: RectDef) -> Self {
        Rect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// A `paths`-scoped override, the strongest of the four resolution layers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Override {
    paths: Vec<String>,
    threshold: Option<f32>,
    tolerance_ratio: Option<f64>,
    #[serde(default)]
    ignore_regions: Vec<RectDef>,
}

/// The top-level shape of `pixeldelta.config.json`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct File {
    threshold: Option<f32>,
    tolerance_ratio: Option<f64>,
    #[serde(default)]
    ignore_regions: Vec<RectDef>,
    #[serde(default)]
    overrides: Vec<Override>,
}

/// The merged base settings plus the parsed per-path overrides.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    base: Settings,
    overrides: Vec<Override>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            threshold: DEFAULT_THRESHOLD,
            tolerance_ratio: DEFAULT_TOLERANCE_RATIO,
            ignore_regions: Vec::new(),
        }
    }
}

impl Config {
    /// The settings that apply when no override matches: the run-level
    /// `threshold` and `toleranceRatio`, and the ignore regions common to
    /// every entry.
    pub fn base(&self) -> &Settings {
        &self.base
    }

    /// Resolves the settings for one entry, applying every override whose
    /// `paths` matches `rel`, in file order (a later match overwrites an
    /// earlier one's `threshold` and `toleranceRatio`; ignore regions from
    /// every match accumulate).
    pub fn settings(&self, rel: &str) -> Settings {
        let rel = rel.replace('\\', "/");
        let mut settings = self.base.clone();
        for over in &self.overrides {
            if !over
                .paths
                .iter()
                .any(|pattern| pattern_match(pattern, &rel))
            {
                continue;
            }
            if let Some(threshold) = over.threshold {
                settings.threshold = threshold;
            }
            if let Some(tolerance_ratio) = over.tolerance_ratio {
                settings.tolerance_ratio = tolerance_ratio;
            }
            settings
                .ignore_regions
                .extend(over.ignore_regions.iter().copied().map(Rect::from));
        }
        settings
    }
}

/// Reasons the config file could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// `--config` names a path that does not exist.
    #[error("{path} does not exist")]
    NotFound { path: PathBuf },
    /// The file exists but could not be read.
    #[error("{path} could not be read: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file is not valid JSON, or does not match the config schema.
    #[error("{path} could not be parsed: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// Default filename read in the working directory when `--config` is not
/// given.
const DEFAULT_FILENAME: &str = "pixeldelta.config.json";

/// Loads the config file and merges it with the flag values, producing the
/// run-level base settings the CLI resolves every entry against.
///
/// `config_path` is `--config`, read as an error if it does not exist.
/// Without it, `working_dir.join("pixeldelta.config.json")` is read if
/// present; parent directories are never searched. `threshold_flag` and
/// `tolerance_ratio_flag` are `--threshold` and `--tolerance-ratio`, and
/// `ignore_regions_flag` is every `--ignore-region` given; all three are
/// `None`/`empty` when the flag was not passed.
pub fn load_config(
    config_path: Option<&Path>,
    working_dir: &Path,
    threshold_flag: Option<f32>,
    tolerance_ratio_flag: Option<f64>,
    ignore_regions_flag: Vec<Rect>,
) -> Result<Config, ConfigError> {
    let file = match config_path {
        Some(path) => {
            if !path.is_file() {
                return Err(ConfigError::NotFound {
                    path: path.to_path_buf(),
                });
            }
            read_file(path)?
        }
        None => {
            let default_path = working_dir.join(DEFAULT_FILENAME);
            if default_path.is_file() {
                read_file(&default_path)?
            } else {
                File::default()
            }
        }
    };

    let mut ignore_regions = ignore_regions_flag;
    ignore_regions.extend(file.ignore_regions.iter().copied().map(Rect::from));

    Ok(Config {
        base: Settings {
            threshold: threshold_flag
                .or(file.threshold)
                .unwrap_or(DEFAULT_THRESHOLD),
            tolerance_ratio: tolerance_ratio_flag
                .or(file.tolerance_ratio)
                .unwrap_or(DEFAULT_TOLERANCE_RATIO),
            ignore_regions,
        },
        overrides: file.overrides,
    })
}

fn read_file(path: &Path) -> Result<File, ConfigError> {
    let bytes = std::fs::read(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Matches a glob-like `pattern` against a `/`-joined relative path.
///
/// A dedicated glob crate is avoided so the CLI's pattern syntax stays its
/// own small contract rather than whatever a crate's implementation happens
/// to support. Three constructs are recognized:
///
/// - `*` matches zero or more characters, never crossing a `/`.
/// - `**`, written as an entire segment (`a/**/b`, `**/x.png`, `dashboard/**`),
///   matches zero or more whole segments. Written inside a segment (`a**b`)
///   it behaves the same as `*`.
/// - `?` matches exactly one character other than `/`.
///
/// Every other character is literal. Matching is anchored at both ends: the
/// whole path must match, not a substring of it. Both `pattern` and `path`
/// have `\` normalized to `/` first, so a pattern written on Windows matches
/// the same paths it would on any other platform.
fn pattern_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern_segments: Vec<&str> = pattern.split('/').collect();
    let path_segments: Vec<&str> = path.split('/').collect();
    segments_match(&pattern_segments, &path_segments)
}

/// Matches a pattern already split on `/` against a path split the same way.
///
/// A `**` segment tries consuming every possible number of path segments
/// (including zero), recursing on each. Any other segment must match exactly
/// one path segment through [`segment_match`].
fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    let Some((&head, rest)) = pattern.split_first() else {
        return path.is_empty();
    };
    if head == "**" {
        return (0..=path.len()).any(|skip| segments_match(rest, &path[skip..]));
    }
    let Some((&first, path_rest)) = path.split_first() else {
        return false;
    };
    segment_match(head, first) && segments_match(rest, path_rest)
}

/// Matches one pattern segment (`*` and `?` wildcards, otherwise literal)
/// against one path segment.
///
/// This is the classic wildcard-matching scan: `*` records a backtrack point
/// and consumes text greedily, backing off one text character at a time when
/// a later literal or `?` fails to match. It runs on `char`s rather than
/// bytes so a multi-byte character is never split.
fn segment_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut backtrack: Option<usize> = None;
    let mut resume_at = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            backtrack = Some(pi);
            resume_at = ti;
            pi += 1;
        } else if let Some(star) = backtrack {
            pi = star + 1;
            resume_at += 1;
            ti = resume_at;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }
    pi == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_does_not_cross_a_slash() {
        assert!(pattern_match("a/*.png", "a/b.png"));
        assert!(!pattern_match("a/*.png", "a/b/c.png"));
    }

    #[test]
    fn double_star_matches_zero_segments() {
        assert!(pattern_match("a/**/b.png", "a/b.png"));
    }

    #[test]
    fn double_star_matches_multiple_segments() {
        assert!(pattern_match("a/**/b.png", "a/x/y/b.png"));
    }

    #[test]
    fn double_star_as_a_whole_segment_at_the_start() {
        assert!(pattern_match("**/clock-1.png", "clock-1.png"));
        assert!(pattern_match("**/clock-1.png", "a/b/clock-1.png"));
    }

    #[test]
    fn double_star_as_a_whole_segment_at_the_end() {
        assert!(pattern_match("dashboard/**", "dashboard"));
        assert!(pattern_match("dashboard/**", "dashboard/a/b.png"));
        assert!(!pattern_match("dashboard/**", "other/a.png"));
    }

    #[test]
    fn double_star_written_inside_a_segment_behaves_as_star() {
        assert!(pattern_match("a**b.png", "aXYZb.png"));
        // It still does not cross a slash, the same as a single `*`.
        assert!(!pattern_match("a**b.png", "a/b.png"));
    }

    #[test]
    fn question_mark_matches_exactly_one_character() {
        assert!(pattern_match("clock-?.png", "clock-1.png"));
        assert!(!pattern_match("clock-?.png", "clock-12.png"));
        assert!(!pattern_match("clock-?.png", "clock-.png"));
    }

    #[test]
    fn literal_characters_match_exactly() {
        assert!(pattern_match("a/b.png", "a/b.png"));
        assert!(!pattern_match("a/b.png", "a/c.png"));
    }

    #[test]
    fn matching_is_anchored_at_both_ends() {
        assert!(!pattern_match("b.png", "a/b.png"));
        assert!(!pattern_match("a/b.png", "a/b.png.bak"));
    }

    #[test]
    fn unknown_top_level_key_is_rejected() {
        let json = r#"{"threshold": 0.2, "typo": true}"#;
        let error = serde_json::from_str::<File>(json);
        assert!(error.is_err(), "a typo'd key should be rejected");
    }

    #[test]
    fn camel_case_keys_deserialize() {
        let json = r#"{
            "threshold": 0.2,
            "toleranceRatio": 0.01,
            "ignoreRegions": [{"x": 1, "y": 2, "width": 3, "height": 4}],
            "overrides": [{"paths": ["a/*.png"], "toleranceRatio": 0.5}]
        }"#;
        let file: File = serde_json::from_str(json).expect("valid camelCase keys parse");
        assert_eq!(file.threshold, Some(0.2));
        assert_eq!(file.tolerance_ratio, Some(0.01));
        assert_eq!(file.ignore_regions.len(), 1);
        assert_eq!(file.overrides.len(), 1);
    }

    #[test]
    fn an_override_without_paths_is_rejected() {
        let json = r#"{"overrides": [{"threshold": 0.2}]}"#;
        assert!(serde_json::from_str::<File>(json).is_err());
    }

    fn config_with(
        top_threshold: Option<f32>,
        flag_threshold: Option<f32>,
        overrides: Vec<Override>,
    ) -> Config {
        Config {
            base: Settings {
                threshold: flag_threshold
                    .or(top_threshold)
                    .unwrap_or(DEFAULT_THRESHOLD),
                tolerance_ratio: DEFAULT_TOLERANCE_RATIO,
                ignore_regions: Vec::new(),
            },
            overrides,
        }
    }

    #[test]
    fn an_override_beats_the_flag_which_beats_the_config_top_level() {
        let config = config_with(
            Some(0.2),
            Some(0.3),
            vec![Override {
                paths: vec!["a/*.png".to_owned()],
                threshold: Some(0.9),
                tolerance_ratio: None,
                ignore_regions: Vec::new(),
            }],
        );

        assert_eq!(config.settings("a/x.png").threshold, 0.9);
        assert_eq!(config.settings("b/x.png").threshold, 0.3);
    }

    #[test]
    fn without_a_flag_the_config_top_level_wins_over_the_default() {
        let config = config_with(Some(0.2), None, Vec::new());
        assert_eq!(config.settings("anything.png").threshold, 0.2);
    }

    #[test]
    fn ignore_regions_are_the_union_of_the_flag_top_level_and_matching_overrides() {
        let flag_region = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        let top_region = RectDef {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        };
        let override_region = RectDef {
            x: 2,
            y: 2,
            width: 1,
            height: 1,
        };
        let config = Config {
            base: Settings {
                threshold: DEFAULT_THRESHOLD,
                tolerance_ratio: DEFAULT_TOLERANCE_RATIO,
                ignore_regions: vec![flag_region, top_region.into()],
            },
            overrides: vec![Override {
                paths: vec!["a/*.png".to_owned()],
                threshold: None,
                tolerance_ratio: None,
                ignore_regions: vec![override_region],
            }],
        };

        let regions = config.settings("a/x.png").ignore_regions;
        assert_eq!(regions.len(), 3);
        assert!(regions.contains(&flag_region));
        assert!(regions.contains(&Rect::from(top_region)));
        assert!(regions.contains(&Rect::from(override_region)));

        // A non-matching path does not pick up the override's region.
        let regions = config.settings("b/x.png").ignore_regions;
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn the_last_matching_override_wins() {
        let config = Config {
            base: Settings::default(),
            overrides: vec![
                Override {
                    paths: vec!["a/*.png".to_owned()],
                    threshold: Some(0.1),
                    tolerance_ratio: None,
                    ignore_regions: Vec::new(),
                },
                Override {
                    paths: vec!["a/*.png".to_owned()],
                    threshold: Some(0.2),
                    tolerance_ratio: None,
                    ignore_regions: Vec::new(),
                },
            ],
        };

        assert_eq!(config.settings("a/x.png").threshold, 0.2);
    }

    #[test]
    fn a_missing_config_path_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-file.json");

        let error = load_config(Some(&missing), dir.path(), None, None, Vec::new())
            .expect_err("a --config path that does not exist is an error");
        assert!(matches!(error, ConfigError::NotFound { .. }));
    }

    #[test]
    fn without_config_the_working_directory_file_is_read_if_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DEFAULT_FILENAME), r#"{"threshold": 0.25}"#).unwrap();

        let config = load_config(None, dir.path(), None, None, Vec::new())
            .expect("the default filename is read");
        assert_eq!(config.base().threshold, 0.25);
    }

    #[test]
    fn without_config_or_a_default_file_no_config_is_used() {
        let dir = tempfile::tempdir().unwrap();

        let config = load_config(None, dir.path(), None, None, Vec::new())
            .expect("no config file is not an error");
        assert_eq!(config.base().threshold, DEFAULT_THRESHOLD);
    }
}
