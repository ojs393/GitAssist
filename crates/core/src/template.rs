pub const BLOCK_BEGIN: &str = "<!-- gitassist:begin -->";
pub const BLOCK_END: &str = "<!-- gitassist:end -->";

/// A template line is a required section when it is a markdown heading
/// (`## What changed`) or a label line ending in a colon (`Why:`). Commit
/// templates should use labels, since git treats `#` lines as comments.
fn section_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.starts_with('#') {
        let name = t.trim_start_matches('#').trim();
        return (!name.is_empty() && !name.contains('{')).then(|| name.to_string());
    }
    if t.len() > 1 && t.ends_with(':') && !t.contains('{') && !t.contains("://") {
        let name = t.trim_end_matches(':').trim();
        let starts_upper = name.chars().next().is_some_and(char::is_uppercase);
        return (starts_upper && name.split_whitespace().count() <= 5).then(|| name.to_string());
    }
    None
}

pub fn required_sections(template: &str) -> Vec<String> {
    template.lines().filter_map(section_name).collect()
}

pub fn missing_sections(text: &str, required: &[String]) -> Vec<String> {
    let present: Vec<String> = text
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches('#')
                .trim_end_matches(':')
                .trim()
                .to_lowercase()
        })
        .collect();
    required
        .iter()
        .filter(|r| !present.contains(&r.to_lowercase()))
        .cloned()
        .collect()
}

/// The last well-formed begin/end pair with no other marker in between, so
/// stray markers typed by hand can never swallow the user's text.
fn managed_span(text: &str) -> Option<(usize, usize)> {
    text.match_indices(BLOCK_BEGIN)
        .filter_map(|(start, _)| {
            let after = start + BLOCK_BEGIN.len();
            let end_rel = text[after..].find(BLOCK_END)?;
            let inner = &text[after..after + end_rel];
            (!inner.contains(BLOCK_BEGIN)).then_some((start, after + end_rel + BLOCK_END.len()))
        })
        .last()
}

/// The text with GitAssist's block taken out (used while length rules run).
/// Stray copies of the markers are dropped too: they are GitAssist's syntax.
pub fn remove_managed_block(text: &str) -> String {
    let without = match managed_span(text) {
        Some((s, e)) => format!("{}\n\n{}", &text[..s], &text[e..]),
        None => text.to_string(),
    };
    crate::text::normalize_whitespace(&without.replace(BLOCK_BEGIN, "").replace(BLOCK_END, ""))
}

pub fn managed_block_text(content: &str) -> String {
    format!("{BLOCK_BEGIN}\n{}\n{BLOCK_END}", content.trim_end())
}

/// Puts the managed block at the bottom of the text, replacing any earlier copy.
pub fn replace_managed_block(text: &str, content: &str) -> String {
    let base = remove_managed_block(text);
    let block = managed_block_text(content);
    if base.is_empty() {
        block
    } else {
        format!("{base}\n\n{block}")
    }
}

pub struct ChangeRow {
    pub area: String,
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
}

pub fn render_change_table(rows: &[ChangeRow]) -> String {
    let mut s = String::from("| Area | Files | Changes |\n|------|------:|--------:|\n");
    for r in rows {
        s.push_str(&format!(
            "| `{}` | {} | +{} / −{} |\n",
            r.area, r.files, r.insertions, r.deletions
        ));
    }
    s.trim_end().to_string()
}

/// Expands `{name}` placeholders; unknown placeholders are left as written.
pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}
