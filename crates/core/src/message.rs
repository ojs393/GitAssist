/// The marker git writes above the diff in `git commit -v`.
const SCISSORS: &str = "------------------------ >8 ------------------------";

/// The file git hands to `commit-msg` still contains comment lines (and the
/// diff after the scissors line, with `-v`). Rules apply only to the real
/// message; comments are re-attached unchanged afterwards.
pub struct GitMessage {
    pub content: String,
    pub comments: String,
}

pub fn split_git_comments(raw: &str, comment_char: char) -> GitMessage {
    let mut content = Vec::new();
    let mut comments = Vec::new();
    let mut after_scissors = false;
    for line in raw.lines() {
        if after_scissors {
            comments.push(line);
            continue;
        }
        if line.starts_with(comment_char) {
            if line.contains(SCISSORS) {
                after_scissors = true;
            }
            comments.push(line);
        } else {
            content.push(line);
        }
    }
    GitMessage {
        content: content.join("\n"),
        comments: comments.join("\n"),
    }
}

pub fn join_git_comments(content: &str, comments: &str) -> String {
    match (content.is_empty(), comments.is_empty()) {
        (_, true) => format!("{content}\n"),
        (true, false) => format!("\n{comments}\n"),
        (false, false) => format!("{content}\n\n{comments}\n"),
    }
}

/// First line and everything after the blank separator line.
pub fn subject_and_body(message: &str) -> (&str, &str) {
    match message.split_once('\n') {
        None => (message, ""),
        Some((subject, rest)) => (subject, rest.trim_start_matches('\n')),
    }
}
