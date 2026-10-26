use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::config::{Conventional, Mode};
use crate::effective::EffectiveRules;
use crate::message::subject_and_body;
use crate::{template, text, trailers};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub rule: &'static str,
    /// `Fix` means the change was applied; `Block`/`Warn` mean it was not.
    pub mode: Mode,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Input<'a> {
    /// GitHub title or `gh pr merge --subject`; `None` for git commit messages.
    pub title: Option<&'a str>,
    /// Full commit message, or a GitHub markdown body.
    pub body: Option<&'a str>,
    /// Current PR body, for the growth rule.
    pub previous_body: Option<&'a str>,
    /// Pre-rendered content for the managed block (PR tabs).
    pub managed_block: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub title: Option<String>,
    pub body: Option<String>,
    pub findings: Vec<Finding>,
    pub changed: bool,
}

impl Outcome {
    pub fn blocked(&self) -> bool {
        self.findings.iter().any(|f| f.mode == Mode::Block)
    }

    pub fn status(&self) -> &'static str {
        if self.blocked() {
            "blocked"
        } else if self.findings.iter().any(|f| f.mode == Mode::Warn) {
            "warned"
        } else if self.changed {
            "fixed"
        } else {
            "clean"
        }
    }
}

/// Rules whose problems cannot be fixed without inventing prose.
fn unfixable(mode: Mode) -> Mode {
    if mode == Mode::Fix {
        Mode::Block
    } else {
        mode
    }
}

fn conventional_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z]+(\([^)]+\))?!?: \S").expect("valid regex"))
}

pub fn process(input: &Input, rules: &EffectiveRules) -> Outcome {
    let original_title = input.title.map(str::to_string);
    let original_body = input.body.map(str::to_string);
    if !rules.enabled {
        return Outcome {
            title: original_title,
            body: original_body,
            findings: vec![],
            changed: false,
        };
    }

    let tab = rules.tab.as_str();
    let commit_style = rules.tab.is_commit_message() && input.title.is_none();
    let mut findings = Vec::new();
    let mut push = |rule: &'static str, mode: Mode, message: String| {
        findings.push(Finding {
            rule,
            mode,
            message: format!("GitAssist [{tab}] {message}"),
        });
    };

    let title = input
        .title
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
    let mut body = input.body.map(str::to_string);

    if let Some(b) = body.as_mut() {
        if rules.strip_trailers && !rules.strip_patterns.is_empty() {
            let (stripped, n) = trailers::strip_lines(b, &rules.strip_patterns);
            if n > 0 {
                let what = if n == 1 {
                    "1 agent co-author line".to_string()
                } else {
                    format!("{n} agent co-author lines")
                };
                match rules.strip_mode {
                    Mode::Fix => {
                        *b = stripped;
                        push("strip_trailers", Mode::Fix, format!("removed {what}."));
                    }
                    m => push(
                        "strip_trailers",
                        m,
                        format!("contains {what}. Remove them and retry."),
                    ),
                }
            }
        }

        *b = text::normalize_whitespace(b);
        if commit_style {
            *b = text::ensure_subject_separator(b);
        }

        // The managed block is set aside while wrap and length rules run, then put back.
        let managed = if rules.managed_table {
            input.managed_block
        } else {
            None
        };
        let block_len = match managed {
            Some(block) => {
                *b = template::remove_managed_block(b);
                2 + template::managed_block_text(block).chars().count()
            }
            None => 0,
        };

        if let Some(w) = &rules.wrap {
            let (head, rest) = split_for_body(b, commit_style);
            let n = text::overlong_lines(&rest, w.value);
            if n > 0 {
                match w.mode {
                    Mode::Fix => {
                        *b = join_body(&head, &text::wrap_body(&rest, w.value));
                        push(
                            "wrap",
                            Mode::Fix,
                            format!("wrapped {n} line(s) at {} chars.", w.value),
                        );
                    }
                    m => push(
                        "wrap",
                        m,
                        format!(
                            "{n} body line(s) exceed {} chars. Wrap them and retry.",
                            w.value
                        ),
                    ),
                }
            }
        }

        // Length rules measure the user's text; GitAssist's own added lines are
        // taken off first and put back last, so re-running never cuts again.
        let wrap_width = rules
            .wrap
            .as_ref()
            .filter(|w| w.mode == Mode::Fix)
            .map(|w| w.value);
        let added: Vec<&str> = rules
            .add_trailers
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        if !added.is_empty() {
            let kept: Vec<&str> = b.lines().filter(|l| !added.contains(&l.trim())).collect();
            *b = text::normalize_whitespace(&kept.join("\n"));
        }
        // Upper bound of what add_trailers appends (blank line + one line each),
        // so truncation lands on the same text no matter how often it runs.
        let trailer_len = if added.is_empty() {
            0
        } else {
            2 + added.iter().map(|t| t.chars().count()).sum::<usize>() + added.len() - 1
        };

        if let Some(max) = &rules.body_max {
            let (head, rest) = split_for_body(b, commit_style);
            let len = rest.chars().count();
            if len > max.value {
                match max.mode {
                    Mode::Fix => {
                        *b = join_body(&head, &text::truncate_within(&rest, max.value, wrap_width));
                        push(
                            "body_max",
                            Mode::Fix,
                            format!("body truncated from {len} to {} chars.", max.value),
                        );
                    }
                    m => push(
                        "body_max",
                        m,
                        format!(
                            "body is {len} chars; limit is {}. Shorten the body and retry.",
                            max.value
                        ),
                    ),
                }
            }
        }

        if let Some(max) = &rules.total_max {
            let title_len = title.as_deref().map(|t| t.chars().count()).unwrap_or(0);
            let len = title_len + b.chars().count() + trailer_len + block_len;
            if len > max.value {
                match max.mode {
                    Mode::Fix => {
                        let (head, rest) = split_for_body(b, commit_style);
                        let head_len = head.chars().count() + if head.is_empty() { 0 } else { 2 };
                        let room = max
                            .value
                            .saturating_sub(title_len + head_len + trailer_len + block_len);
                        *b = join_body(&head, &text::truncate_within(&rest, room, wrap_width));
                        push(
                            "total_max",
                            Mode::Fix,
                            format!("message cut from {len} to {} chars.", max.value),
                        );
                        if title_len + head_len + trailer_len + block_len >= max.value {
                            push(
                                "total_max",
                                Mode::Warn,
                                format!("the title, subject, added lines and change table alone exceed {} chars.", max.value),
                            );
                        }
                    }
                    m => push(
                        "total_max",
                        m,
                        format!(
                            "message is {len} chars in total; limit is {}. Condense it and retry.",
                            max.value
                        ),
                    ),
                }
            }
        }

        if let Some(block) = managed {
            *b = template::replace_managed_block(b, block);
        }
        if !added.is_empty() && !b.trim().is_empty() {
            *b = trailers::add_trailers(b, &rules.add_trailers);
        }
    }

    let subject: Option<String> = if commit_style {
        body.as_deref().map(|b| subject_and_body(b).0.to_string())
    } else {
        title.clone()
    };
    let has_content = subject.as_deref().is_some_and(|s| !s.trim().is_empty())
        || body.as_deref().is_some_and(|b| !b.trim().is_empty());

    if has_content {
        if let (Some(max), Some(s)) = (&rules.subject_max, subject.as_deref()) {
            let len = s.chars().count();
            if len > max.value {
                let noun = if commit_style { "subject" } else { "title" };
                push(
                    "subject_max",
                    unfixable(max.mode),
                    format!(
                        "{noun} is {len} chars; limit is {}. Shorten the {noun} and retry.",
                        max.value
                    ),
                );
            }
        }

        if let Some(s) = subject.as_deref() {
            let is_conv = conventional_re().is_match(s);
            let mode = unfixable(rules.conventional.mode);
            match rules.conventional.value {
                Conventional::Required if !is_conv => push(
                    "conventional",
                    mode,
                    "subject must use a conventional prefix like `feat: ...` or `fix(scope): ...`."
                        .into(),
                ),
                Conventional::Forbidden if is_conv => push(
                    "conventional",
                    mode,
                    "subject must not use a conventional `type:` prefix.".into(),
                ),
                _ => {}
            }
        }

        if let Some(b) = body.as_deref() {
            if !rules.required_sections.is_empty() {
                let missing = template::missing_sections(b, &rules.required_sections);
                if !missing.is_empty() {
                    push(
                        "sections",
                        unfixable(rules.sections_mode),
                        format!(
                            "missing required section(s): {}. Add them and retry.",
                            missing.join(", ")
                        ),
                    );
                }
            }
        }

        let haystack = format!(
            "{}\n{}",
            title.as_deref().unwrap_or(""),
            body.as_deref().unwrap_or("")
        )
        .to_lowercase();
        let banned: Vec<&str> = rules
            .banned_phrases
            .value
            .iter()
            .map(String::as_str)
            .filter(|p| !p.trim().is_empty() && haystack.contains(&p.to_lowercase()))
            .collect();
        if !banned.is_empty() {
            push(
                "banned_phrases",
                unfixable(rules.banned_phrases.mode),
                format!(
                    "contains banned phrase(s): {}. Reword and retry.",
                    banned.join(", ")
                ),
            );
        }

        if let (Some(max), Some(prev), Some(b)) =
            (&rules.growth_max, input.previous_body, body.as_deref())
        {
            let grew = b.chars().count().saturating_sub(prev.chars().count());
            if grew > max.value {
                push(
                    "growth_max",
                    unfixable(max.mode),
                    format!("description grew by {grew} chars; limit per update is {}. Condense it and retry.", max.value),
                );
            }
        }
    }

    let changed = title != original_title || body != original_body;
    Outcome {
        title,
        body,
        findings,
        changed,
    }
}

fn split_for_body(message: &str, commit_style: bool) -> (String, String) {
    if !commit_style {
        return (String::new(), message.to_string());
    }
    let (subject, body) = subject_and_body(message);
    (subject.to_string(), body.to_string())
}

fn join_body(head: &str, body: &str) -> String {
    match (head.is_empty(), body.is_empty()) {
        (true, _) => body.to_string(),
        (false, true) => head.to_string(),
        (false, false) => format!("{head}\n\n{body}"),
    }
}
