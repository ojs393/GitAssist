use std::fs;
use std::path::Path;

use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub struct Markers {
    pub begin: &'static str,
    pub end: &'static str,
}

pub const MD: Markers = Markers {
    begin: "<!-- gitassist:begin -->",
    end: "<!-- gitassist:end -->",
};
pub const SHELL: Markers = Markers {
    begin: "# >>> gitassist >>>",
    end: "# <<< gitassist <<<",
};

fn find(text: &str, m: Markers) -> Option<(usize, usize)> {
    let start = text.find(m.begin)?;
    let end_rel = text[start..].find(m.end)?;
    let mut end = start + end_rel + m.end.len();
    if text[end..].starts_with("\r\n") {
        end += 2;
    } else if text[end..].starts_with('\n') {
        end += 1;
    }
    Some((start, end))
}

pub fn contains(text: &str, m: Markers) -> bool {
    find(text, m).is_some()
}

pub fn block_content(text: &str, m: Markers) -> Option<String> {
    let (s, e) = find(text, m)?;
    let inner = &text[s + m.begin.len()..e];
    Some(
        inner
            .trim_end()
            .trim_end_matches(m.end)
            .trim_matches('\n')
            .to_string(),
    )
}

/// Replaces the existing block or appends one, separated from prior text by a blank line.
pub fn upsert(text: &str, m: Markers, content: &str) -> String {
    let block = format!("{}\n{}\n{}\n", m.begin, content.trim_end(), m.end);
    if let Some((s, e)) = find(text, m) {
        return format!("{}{}{}", &text[..s], block, &text[e..]);
    }
    if text.is_empty() {
        return block;
    }
    let sep = if text.ends_with("\n\n") {
        ""
    } else if text.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    format!("{text}{sep}{block}")
}

/// Removes the block and the blank-line separator `upsert` added before it.
pub fn remove(text: &str, m: Markers) -> String {
    let Some((s, e)) = find(text, m) else {
        return text.to_string();
    };
    let before = &text[..s];
    let after = &text[e..];
    let before = if after.is_empty() {
        if let Some(b) = before.strip_suffix("\n\n") {
            format!("{b}\n")
        } else {
            before.to_string()
        }
    } else {
        before.to_string()
    };
    format!("{before}{after}")
}

/// Returns true when the file did not exist before.
pub fn upsert_file(path: &Path, m: Markers, content: &str) -> Result<bool> {
    let existing = fs::read_to_string(path).ok();
    let created = existing.is_none();
    let new = upsert(existing.as_deref().unwrap_or(""), m, content);
    if existing.as_deref() != Some(new.as_str()) {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, new)?;
    }
    Ok(created)
}

/// Deletes the file when GitAssist created it and nothing else is left in it.
pub fn remove_file_block(path: &Path, m: Markers, created_by_us: bool) -> Result<()> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };
    let new = remove(&text, m);
    if created_by_us && new.trim().is_empty() {
        fs::remove_file(path)?;
    } else if new != text {
        fs::write(path, new)?;
    }
    Ok(())
}
