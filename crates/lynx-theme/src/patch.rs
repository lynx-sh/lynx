/// Dot-path TOML mutation engine for `lx theme patch`.
///
/// All public functions operate on raw TOML strings (read from / written back to disk).
/// They do NOT validate the resulting theme — callers must run `load_from_path` and
/// restore the snapshot on failure (snapshot/validate/rollback pattern, D-007).
use anyhow::Result;
use lynx_core::error::LynxError;

// ─── Scalar patch ────────────────────────────────────────────────────────────

/// Apply a dot-path scalar mutation to raw TOML content.
///
/// `dot_path` is a dot-separated key sequence e.g. `"colors.accent"` or
/// `"segment.dir.color.fg"`. The target leaf is set to `value` as a TOML string.
/// Intermediate tables are created if absent.
pub fn apply_patch(content: &str, dot_path: &str, value: &str) -> Result<String> {
    let mut root: toml::Value = toml::from_str(content)
        .map_err(|e| anyhow::Error::from(LynxError::Theme(format!("TOML parse error: {e}"))))?;

    let parts: Vec<&str> = dot_path.split('.').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(LynxError::Theme("patch path must not be empty".into()).into());
    }

    set_scalar_at_path(&mut root, &parts, value)?;
    toml::to_string_pretty(&root).map_err(|e| anyhow::Error::from(LynxError::Theme(format!("TOML serialise error: {e}"))))
}

fn set_scalar_at_path(node: &mut toml::Value, parts: &[&str], value: &str) -> Result<()> {
    let key = parts[0];
    if parts.len() == 1 {
        match node {
            toml::Value::Table(t) => {
                t.insert(key.to_string(), toml::Value::String(value.to_string()));
                Ok(())
            }
            _ => Err(LynxError::Theme(format!("cannot set key '{key}' on a non-table value")).into()),
        }
    } else {
        match node {
            toml::Value::Table(t) => {
                let child = t
                    .entry(key.to_string())
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                set_scalar_at_path(child, &parts[1..], value)
            }
            _ => Err(LynxError::Theme(format!("path traverses a non-table value at '{key}'")).into()),
        }
    }
}

// ─── Array ops ───────────────────────────────────────────────────────────────

/// Operations on a TOML string array at a dot-path location.
pub enum ArrayOp {
    /// Append `item` if not already present.
    Append(String),
    /// Remove all occurrences of `item`.
    Remove(String),
    /// Move `item` to immediately after `after`; if `after` not found, append.
    MoveAfter { item: String, after: String },
    /// Move `item` to the front of the array.
    MoveToFront(String),
}

/// Apply an array operation to the value at `dot_path` in raw TOML content.
///
/// The target value at `dot_path` must be a TOML array of strings.
pub fn apply_array_op(content: &str, dot_path: &str, op: ArrayOp) -> Result<String> {
    let mut root: toml::Value = toml::from_str(content)
        .map_err(|e| anyhow::Error::from(LynxError::Theme(format!("TOML parse error: {e}"))))?;

    let parts: Vec<&str> = dot_path.split('.').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(LynxError::Theme("array path must not be empty".into()).into());
    }

    apply_op_at_path(&mut root, &parts, op)?;
    toml::to_string_pretty(&root).map_err(|e| anyhow::Error::from(LynxError::Theme(format!("TOML serialise error: {e}"))))
}

fn apply_op_at_path(node: &mut toml::Value, parts: &[&str], op: ArrayOp) -> Result<()> {
    let key = parts[0];
    if parts.len() == 1 {
        match node {
            toml::Value::Table(t) => {
                let arr = t
                    .get_mut(key)
                    .ok_or_else(|| anyhow::Error::from(LynxError::Theme(format!("path '{key}' not found in TOML"))))?;
                apply_op_to_array(arr, op)
            }
            _ => Err(LynxError::Theme(format!("cannot index non-table at '{key}'")).into()),
        }
    } else {
        match node {
            toml::Value::Table(t) => {
                let child = t
                    .get_mut(key)
                    .ok_or_else(|| anyhow::Error::from(LynxError::Theme(format!("path segment '{key}' not found in TOML"))))?;
                apply_op_at_path(child, &parts[1..], op)
            }
            _ => Err(LynxError::Theme(format!("path traverses non-table at '{key}'")).into()),
        }
    }
}

fn apply_op_to_array(node: &mut toml::Value, op: ArrayOp) -> Result<()> {
    let arr = match node {
        toml::Value::Array(a) => a,
        _ => return Err(LynxError::Theme("target path does not point to an array".into()).into()),
    };

    match op {
        ArrayOp::Append(item) => {
            if !arr.iter().any(|x| x.as_str() == Some(&item)) {
                arr.push(toml::Value::String(item));
            }
        }
        ArrayOp::Remove(item) => {
            arr.retain(|x| x.as_str() != Some(&item));
        }
        ArrayOp::MoveAfter { item, after } => {
            arr.retain(|x| x.as_str() != Some(&item));
            let pos = arr
                .iter()
                .position(|x| x.as_str() == Some(&after))
                .map(|i| i + 1)
                .unwrap_or(arr.len());
            arr.insert(pos, toml::Value::String(item));
        }
        ArrayOp::MoveToFront(item) => {
            arr.retain(|x| x.as_str() != Some(&item));
            arr.insert(0, toml::Value::String(item));
        }
    }
    Ok(())
}

// ─── Segment order helpers ────────────────────────────────────────────────────

/// Which side of the prompt a segment lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn dot_path(self) -> &'static str {
        match self {
            Side::Left => "segments.left.order",
            Side::Right => "segments.right.order",
        }
    }

    pub fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

impl std::str::FromStr for Side {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "left" => Ok(Side::Left),
            "right" => Ok(Side::Right),
            other => Err(LynxError::Theme(format!("expected 'left' or 'right', got '{other}'")).into()),
        }
    }
}

/// Add a segment to the given side, optionally after another segment.
/// If the segment already exists on that side, this is a no-op.
pub fn segment_add(content: &str, name: &str, side: Side, after: Option<&str>) -> Result<String> {
    let op = match after {
        Some(a) => ArrayOp::MoveAfter {
            item: name.to_string(),
            after: a.to_string(),
        },
        None => ArrayOp::Append(name.to_string()),
    };
    apply_array_op(content, side.dot_path(), op)
}

/// Remove a segment from both sides (it may only be on one).
pub fn segment_remove(content: &str, name: &str) -> Result<String> {
    let after_left = apply_array_op(
        content,
        Side::Left.dot_path(),
        ArrayOp::Remove(name.to_string()),
    )?;
    apply_array_op(
        &after_left,
        Side::Right.dot_path(),
        ArrayOp::Remove(name.to_string()),
    )
}

/// Move a segment to `target` side (removes from the other side first).
/// If `after` is given, inserts after that segment; otherwise appends.
pub fn segment_move(content: &str, name: &str, target: Side, after: Option<&str>) -> Result<String> {
    // Remove from the other side (no-op if absent).
    let removed = apply_array_op(
        content,
        target.other().dot_path(),
        ArrayOp::Remove(name.to_string()),
    )?;
    // Add to target side.
    segment_add(&removed, name, target, after)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_TOML: &str = r##"
[meta]
name = "test"
description = ""

[colors]
accent = "#7aa2f7"

[segments.left]
order = ["dir", "git_branch"]

[segments.right]
order = ["cmd_duration"]

[segment.dir]
color = { fg = "$accent" }
"##;

    #[test]
    fn patch_scalar_existing_key() {
        let result = apply_patch(BASE_TOML, "colors.accent", "#ff0000").unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            v["colors"]["accent"].as_str(),
            Some("#ff0000"),
            "accent should be updated"
        );
    }

    #[test]
    fn patch_creates_nested_key() {
        let result = apply_patch(BASE_TOML, "colors.danger", "#f7768e").unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(v["colors"]["danger"].as_str(), Some("#f7768e"));
    }

    #[test]
    fn patch_deep_nested_path() {
        let result = apply_patch(BASE_TOML, "segment.dir.color.fg", "blue").unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(v["segment"]["dir"]["color"]["fg"].as_str(), Some("blue"));
    }

    #[test]
    fn patch_empty_path_errors() {
        assert!(apply_patch(BASE_TOML, "", "val").is_err());
    }

    #[test]
    fn array_append_new() {
        let result =
            apply_array_op(BASE_TOML, "segments.left.order", ArrayOp::Append("venv".into()))
                .unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let arr: Vec<&str> = v["segments"]["left"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(arr, vec!["dir", "git_branch", "venv"]);
    }

    #[test]
    fn array_append_no_duplicate() {
        let result =
            apply_array_op(BASE_TOML, "segments.left.order", ArrayOp::Append("dir".into()))
                .unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let arr = v["segments"]["left"]["order"].as_array().unwrap();
        assert_eq!(arr.iter().filter(|x| x.as_str() == Some("dir")).count(), 1);
    }

    #[test]
    fn array_remove() {
        let result = apply_array_op(
            BASE_TOML,
            "segments.left.order",
            ArrayOp::Remove("git_branch".into()),
        )
        .unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let arr: Vec<&str> = v["segments"]["left"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(arr, vec!["dir"]);
    }

    #[test]
    fn array_move_after() {
        let result = apply_array_op(
            BASE_TOML,
            "segments.left.order",
            ArrayOp::MoveAfter {
                item: "dir".into(),
                after: "git_branch".into(),
            },
        )
        .unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let arr: Vec<&str> = v["segments"]["left"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(arr, vec!["git_branch", "dir"]);
    }

    #[test]
    fn segment_add_appends_to_side() {
        let result = segment_add(BASE_TOML, "venv", Side::Right, None).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let arr: Vec<&str> = v["segments"]["right"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(arr, vec!["cmd_duration", "venv"]);
    }

    #[test]
    fn segment_remove_clears_both_sides() {
        let result = segment_remove(BASE_TOML, "dir").unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let left: Vec<&str> = v["segments"]["left"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(left, vec!["git_branch"]);
    }

    #[test]
    fn segment_move_switches_sides() {
        let result = segment_move(BASE_TOML, "git_branch", Side::Right, None).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let left: Vec<&str> = v["segments"]["left"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        let right: Vec<&str> = v["segments"]["right"]["order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert!(!left.contains(&"git_branch"), "should no longer be on left");
        assert!(right.contains(&"git_branch"), "should now be on right");
    }

    #[test]
    fn side_from_str() {
        assert_eq!("left".parse::<Side>().unwrap(), Side::Left);
        assert_eq!("RIGHT".parse::<Side>().unwrap(), Side::Right);
        assert!("top".parse::<Side>().is_err());
    }
}
