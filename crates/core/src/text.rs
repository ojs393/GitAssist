/// Trims trailing whitespace, removes leading/trailing blank lines and
/// collapses runs of blank lines to one. Fenced code blocks are left alone.
pub fn normalize_whitespace(text: &str) -> String {
    // Windows line endings and stray carriage returns never belong in git text.
    let text = text.replace('\r', "");
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    let mut blank_run = 0;
    for raw in text.lines() {
        if raw.trim_start().starts_with("```") {
            in_fence = !in_fence;
            blank_run = 0;
            out.push(raw.trim_end().to_string());
            continue;
        }
        if in_fence {
            out.push(raw.to_string());
            continue;
        }
        let line = raw.trim_end();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 || out.is_empty() {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push(line.to_string());
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Git convention: the subject is separated from the body by one blank line.
pub fn ensure_subject_separator(message: &str) -> String {
    let mut lines = message.lines();
    let Some(subject) = lines.next() else {
        return String::new();
    };
    let rest: Vec<&str> = lines.collect();
    match rest.first() {
        None => subject.to_string(),
        Some(&"") => message.to_string(),
        Some(_) => format!("{subject}\n\n{}", rest.join("\n")),
    }
}

fn is_unwrappable(line: &str) -> bool {
    let t = line.trim_start();
    line.starts_with("    ")
        || line.starts_with('\t')
        || t.starts_with('|')
        || t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with("<!--")
        || crate::trailers::is_git_trailer(line)
}

fn list_indent(line: &str) -> usize {
    let lead = line.len() - line.trim_start().len();
    let t = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if t.starts_with(marker) {
            return lead + 2;
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")) {
        return lead + digits + 2;
    }
    lead
}

/// Splits lines longer than `width` at word boundaries. Short lines are never
/// joined, so hand-made layout survives; code fences, tables, quotes, headings
/// and trailers are untouched; a single word longer than `width` (a URL, say)
/// stays whole on its own line.
pub fn wrap_body(text: &str, width: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push(line.to_string());
            continue;
        }
        if in_fence || line.chars().count() <= width || is_unwrappable(line) {
            out.push(line.to_string());
            continue;
        }
        let indent = " ".repeat(list_indent(line));
        let lead = &line[..line.len() - line.trim_start().len()];
        let mut current = lead.to_string();
        let mut current_has_word = false;
        for word in line.split_whitespace() {
            let candidate_len =
                current.chars().count() + usize::from(current_has_word) + word.chars().count();
            if current_has_word && candidate_len > width {
                out.push(std::mem::replace(&mut current, indent.clone()));
                current_has_word = false;
            }
            if current_has_word {
                current.push(' ');
            }
            current.push_str(word);
            current_has_word = true;
        }
        out.push(current);
    }
    out.join("\n")
}

/// Lines of the body that exceed `width` and would be wrapped by [`wrap_body`].
pub fn overlong_lines(text: &str, width: usize) -> usize {
    let mut in_fence = false;
    text.lines()
        .filter(|line| {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                return false;
            }
            !in_fence
                && line.chars().count() > width
                && !is_unwrappable(line)
                && line.split_whitespace().count() > 1
        })
        .count()
}

/// Like [`truncate_chars`], but when lines are wrapped at `width` the final
/// line (with its `…`) is kept within the width too.
pub fn truncate_within(text: &str, max: usize, width: Option<usize>) -> String {
    let s = truncate_chars(text, max);
    let Some(w) = width.filter(|w| *w > 1) else {
        return s;
    };
    if !s.ends_with('…') {
        return s;
    }
    let (head, last) = match s.rfind('\n') {
        Some(i) => (&s[..=i], &s[i + 1..]),
        None => ("", s.as_str()),
    };
    if last.chars().count() <= w {
        return s;
    }
    let body: String = last.trim_end_matches('…').chars().take(w - 1).collect();
    let lead = body.len() - body.trim_start().len();
    let cut = match body.rfind(char::is_whitespace).filter(|i| *i > lead) {
        Some(i) => body[..i].trim_end().to_string(),
        None => body,
    };
    format!("{head}{cut}…")
}

pub fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s = s.trim_end().to_string();
    s.push('…');
    s
}
