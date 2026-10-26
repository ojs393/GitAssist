use gitassist_core::config::default_strip_patterns;
use gitassist_core::effective::compile_patterns;
use gitassist_core::message::{join_git_comments, split_git_comments};
use gitassist_core::template::{missing_sections, replace_managed_block, required_sections};
use gitassist_core::text::{normalize_whitespace, wrap_body};
use gitassist_core::trailers::{add_trailers, strip_lines};

const CLAUDE_MSG: &str = "feat: add parser\n\nAdds the parser.\n\n🤖 Generated with [Claude Code](https://claude.com/claude-code)\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";

#[test]
fn strips_agent_attribution() {
    let (out, n) = strip_lines(CLAUDE_MSG, &compile_patterns(&default_strip_patterns()));
    assert_eq!(n, 2);
    assert_eq!(
        normalize_whitespace(&out),
        "feat: add parser\n\nAdds the parser."
    );
}

#[test]
fn strips_cursor_and_aider_coauthors() {
    let msg = "fix: x\n\nbody\n\nCo-authored-by: Cursor <cursoragent@cursor.com>\nCo-authored-by: aider (gpt) <noreply@aider.chat>";
    let (out, n) = strip_lines(msg, &compile_patterns(&default_strip_patterns()));
    assert_eq!(n, 2);
    assert_eq!(out, "fix: x\n\nbody");
}

#[test]
fn add_trailers_is_idempotent_and_joins_block() {
    let t = vec!["Signed-off-by: Me <me@x>".to_string()];
    let once = add_trailers("fix: x\n\nbody", &t);
    assert_eq!(once, "fix: x\n\nbody\n\nSigned-off-by: Me <me@x>");
    assert_eq!(add_trailers(&once, &t), once);
    let two = add_trailers(&once, &["Reviewed-by: You <y@x>".to_string()]);
    assert_eq!(
        two,
        "fix: x\n\nbody\n\nSigned-off-by: Me <me@x>\nReviewed-by: You <y@x>"
    );
}

#[test]
fn normalize_collapses_blank_runs_but_not_code() {
    let input = "\n\nsubject  \n\n\n\nbody\n```\na  \n\n\nb\n```\n\n\n";
    assert_eq!(
        normalize_whitespace(input),
        "subject\n\nbody\n```\na  \n\n\nb\n```"
    );
}

#[test]
fn wrap_splits_long_prose_only() {
    let long = "This sentence is deliberately long so that it has to be wrapped at forty chars.";
    let table = "| a very long table row that must stay on one single line no matter what |";
    let url = "https://example.com/a/really/long/url/that/should/never/be/broken/up/at/all";
    let fence =
        "```\nlet x = \"a code line that is far too long to fit but is code so it stays\";\n```";
    let list = "- a list item that is long enough that it needs to wrap onto the next line";
    let input = format!("{long}\n{table}\n{url}\n{fence}\n{list}\nshort line\nCo-authored-by: Someone With A Long Name <someone@example.com>");
    let out = wrap_body(&input, 40);
    for line in out.lines() {
        let protected = line.starts_with('|')
            || line.starts_with("https://")
            || line.contains("let x")
            || line.starts_with("Co-authored-by");
        assert!(
            protected || line.chars().count() <= 40,
            "line too long: {line:?}"
        );
    }
    assert!(out.contains(table));
    assert!(out.contains(url));
    assert!(out.contains("short line"));
    assert!(out.contains("\n  "), "list continuation is indented: {out}");
    assert_eq!(wrap_body(&out, 40), out, "idempotent");
}

#[test]
fn sections_from_template() {
    let tpl = "{type}: {summary}\n\nWhy:\n{body}\n\n## Testing\n{tests}";
    assert_eq!(
        required_sections(tpl),
        vec!["Why".to_string(), "Testing".to_string()]
    );
    let req = required_sections(tpl);
    assert_eq!(
        missing_sections("fix: x\n\nwhy:\nbecause", &req),
        vec!["Testing".to_string()]
    );
}

#[test]
fn managed_block_is_kept_once_at_the_bottom() {
    let first = replace_managed_block("Intro text", "| a |");
    assert_eq!(
        first,
        "Intro text\n\n<!-- gitassist:begin -->\n| a |\n<!-- gitassist:end -->"
    );
    let edited = format!("{first}\n\nFooter");
    let second = replace_managed_block(&edited, "| b |");
    assert_eq!(
        second,
        "Intro text\n\nFooter\n\n<!-- gitassist:begin -->\n| b |\n<!-- gitassist:end -->"
    );
    assert_eq!(replace_managed_block(&second, "| b |"), second);
    let stray = replace_managed_block("Keep <!-- gitassist:begin --> me", "| c |");
    assert_eq!(
        stray,
        "Keep  me\n\n<!-- gitassist:begin -->\n| c |\n<!-- gitassist:end -->"
    );
}

#[test]
fn git_comments_split_and_rejoin() {
    let raw = "fix: x\n\nbody\n# Please enter the commit message\n# ------------------------ >8 ------------------------\ndiff --git a b\n+line\n";
    let m = split_git_comments(raw, '#');
    assert_eq!(m.content, "fix: x\n\nbody");
    assert!(m.comments.ends_with("+line"));
    let joined = join_git_comments(&m.content, &m.comments);
    assert!(joined.starts_with("fix: x\n\nbody\n\n# Please"));
}
