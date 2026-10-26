use regex::Regex;

/// Removes every line matching any pattern. Agent signatures are not always in
/// the trailer block (e.g. "🤖 Generated with ..." sits above it), so the whole
/// text is scanned. Returns the new text and how many lines were removed.
pub fn strip_lines(text: &str, patterns: &[Regex]) -> (String, usize) {
    let mut removed = 0;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let hit = patterns.iter().any(|re| re.is_match(line));
            if hit {
                removed += 1;
            }
            !hit
        })
        .collect();
    if removed == 0 {
        return (text.to_string(), 0);
    }
    (kept.join("\n").trim_end().to_string(), removed)
}

pub fn is_trailer_line(line: &str) -> bool {
    let Some((key, value)) = line.split_once(':') else {
        return false;
    };
    !key.is_empty()
        && !value.trim().is_empty()
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Stricter than [`is_trailer_line`]: prose like "Note: ..." is not a trailer.
pub fn is_git_trailer(line: &str) -> bool {
    if !is_trailer_line(line) {
        return false;
    }
    let key = line.split_once(':').map(|(k, _)| k).unwrap_or_default();
    key.contains('-') || ["Fixes", "Closes", "Resolves", "Refs", "Ref", "See"].contains(&key)
}

/// Appends trailers that are not already present, joining an existing trailer
/// block at the end of the text when there is one.
pub fn add_trailers(text: &str, trailers: &[String]) -> String {
    let existing: Vec<&str> = text.lines().map(str::trim).collect();
    let missing: Vec<&str> = trailers
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty() && !existing.contains(t))
        .collect();
    if missing.is_empty() {
        return text.to_string();
    }
    let base = text.trim_end();
    if base.is_empty() {
        return missing.join("\n");
    }
    let last_paragraph = base.rsplit("\n\n").next().unwrap_or(base);
    let ends_with_trailers = base.contains("\n\n") && last_paragraph.lines().all(is_trailer_line);
    let sep = if ends_with_trailers { "\n" } else { "\n\n" };
    format!("{base}{sep}{}", missing.join("\n"))
}
