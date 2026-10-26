//! Randomised stress test: thousands of generated messages and rule settings,
//! checked against properties that must always hold.

use gitassist_core::config::{Config, Conventional, Mode, RuleSetting};
use gitassist_core::effective::effective;
use gitassist_core::rules::{process, Input};
use gitassist_core::tab::Tab;
use gitassist_core::text::overlong_lines;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len())]
    }
}

const FRAGMENTS: &[&str] = &[
    "fix the thing",
    "feat(api): add retries",
    "Co-authored-by: Claude <noreply@anthropic.com>",
    "Co-Authored-By: Cursor <cursoragent@cursor.com>",
    "Made-with: Cursor",
    "🤖 Generated with [Claude Code](https://claude.com/claude-code)",
    "Signed-off-by: Someone <s@example.com>",
    "",
    "",
    "   ",
    "\t indented code line that is quite long and keeps going past any sensible width",
    "```",
    "let x = \"a long code line inside a fence that must never be wrapped by anything\";",
    "| col | another column | and a third one that makes this table row long |",
    "## Heading",
    "Why:",
    "- list item that goes on and on so that it definitely needs wrapping somewhere soon",
    "1. numbered item with enough words to wrap at a narrow width setting here",
    "https://example.com/a/really/long/url/that/is/one/word/and/must/not/break/anywhere",
    "ünïcödé text with émojis 🎉🎉 and CJK 漢字かな mixed in with plain words here",
    "word ",
    "> quoted reply line",
    "<!-- gitassist:begin -->",
    "<!-- gitassist:end -->",
    "# git comment line",
    "trailing spaces   ",
    "\r",
    "Note: this is prose with a colon, not a trailer, and it is long enough to wrap",
];

fn message(r: &mut Rng) -> String {
    let n = 1 + r.below(14);
    let mut lines: Vec<String> = Vec::new();
    for _ in 0..n {
        let mut l = r.pick(FRAGMENTS).to_string();
        if r.below(6) == 0 {
            l = l.repeat(1 + r.below(4));
        }
        lines.push(l);
    }
    let sep = if r.below(5) == 0 { "\r\n" } else { "\n" };
    lines.join(sep)
}

fn mode(r: &mut Rng) -> Mode {
    [Mode::Fix, Mode::Warn, Mode::Block][r.below(3)]
}

fn config(r: &mut Rng, tab: Tab) -> Config {
    let mut cfg = Config::default();
    cfg.global.trailers.strip_mode = mode(r);
    if r.below(3) == 0 {
        cfg.global.trailers.add = vec!["Signed-off-by: Me <me@example.com>".into()];
    }
    let t = cfg.tabs.get_mut(tab);
    let lim = |r: &mut Rng| [0usize, 1, 10, 40, 72, 500][r.below(6)];
    t.subject_max = Some(RuleSetting::new(lim(r), mode(r)));
    t.body_max = Some(RuleSetting::new(lim(r), mode(r)));
    t.total_max = Some(RuleSetting::new(lim(r), mode(r)));
    t.wrap = Some(RuleSetting::new([0usize, 20, 50, 72][r.below(4)], mode(r)));
    t.conventional = Some(RuleSetting::new(
        [
            Conventional::Off,
            Conventional::Required,
            Conventional::Forbidden,
        ][r.below(3)],
        mode(r),
    ));
    if r.below(3) == 0 {
        t.template = Some("{type}: {summary}\n\nWhy:\n{body}".into());
    }
    if r.below(3) == 0 {
        t.banned_phrases = Some(RuleSetting::new(
            vec!["thing".into(), "漢字".into()],
            mode(r),
        ));
    }
    if r.below(4) == 0 {
        t.managed_table = Some(true);
    }
    cfg
}

#[test]
fn rules_engine_holds_its_invariants_under_random_input() {
    // FUZZ_SEED / FUZZ_ITERS let CI or a developer run deeper searches.
    let seed = std::env::var("FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x9E37_79B9_7F4A_7C15u64);
    let iters = std::env::var("FUZZ_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6000);
    let mut r = Rng(seed.max(1));
    let patterns = gitassist_core::effective::compile_patterns(
        &gitassist_core::config::default_strip_patterns(),
    );
    for i in 0..iters {
        let tab = Tab::ALL[r.below(Tab::ALL.len())];
        let cfg = config(&mut r, tab);
        let rules = effective(&cfg, tab, None);
        let body = message(&mut r);
        let title = (!tab.is_commit_message() && tab.has_title())
            .then(|| message(&mut r).lines().next().unwrap_or("").to_string());
        let prev = (r.below(2) == 0).then(|| message(&mut r));
        let input = Input {
            title: title.as_deref(),
            body: Some(&body),
            previous_body: prev.as_deref(),
            managed_block: Some("| a | b |\n|---|---|"),
        };
        let out = process(&input, &rules);
        let text = out.body.clone().unwrap_or_default();
        let ctx = || {
            format!("case {i} tab {tab:?} body_max={:?} total_max={:?} wrap={:?} title={:?} out_title={:?}\ninput: {body:?}\noutput: {text:?}", rules.body_max, rules.total_max, rules.wrap, title, out.title)
        };

        assert!(
            !text.contains('\r'),
            "no carriage returns survive: {}",
            ctx()
        );

        if rules.strip_trailers && rules.strip_mode == Mode::Fix {
            for line in text.lines() {
                let added = rules.add_trailers.iter().any(|t| t == line);
                assert!(
                    added || !patterns.iter().any(|p| p.is_match(line)),
                    "attribution left behind: {}",
                    ctx()
                );
            }
        }

        if let Some(w) = rules.wrap.as_ref().filter(|w| w.mode == Mode::Fix) {
            let target = if tab.is_commit_message() {
                text.split_once("\n\n").map(|(_, b)| b).unwrap_or("")
            } else {
                &text
            };
            let checked: String = target
                .lines()
                .filter(|l| !rules.add_trailers.iter().any(|t| t == l))
                .collect::<Vec<_>>()
                .join("\n");
            if rules.total_max.as_ref().is_none_or(|t| t.mode != Mode::Fix)
                && rules.body_max.as_ref().is_none_or(|b| b.mode != Mode::Fix)
            {
                assert_eq!(
                    overlong_lines(&checked, w.value),
                    0,
                    "wrap left long lines: {}",
                    ctx()
                );
            }
        }

        if let Some(t) = rules.total_max.as_ref().filter(|t| {
            t.mode == Mode::Fix
                && !out
                    .findings
                    .iter()
                    .any(|f| f.rule == "total_max" && f.mode == Mode::Warn)
        }) {
            let title_len = out.title.as_deref().map(|s| s.chars().count()).unwrap_or(0);
            let subject_len = if tab.is_commit_message() {
                text.lines()
                    .next()
                    .map(|s| s.chars().count() + 2)
                    .unwrap_or(0)
            } else {
                0
            };
            if title_len + subject_len < t.value {
                assert!(
                    title_len + text.chars().count() <= t.value,
                    "total limit exceeded: {}",
                    ctx()
                );
            }
        }

        // Applying the rules again must not change anything further.
        let again = process(
            &Input {
                title: out.title.as_deref(),
                body: out.body.as_deref(),
                ..input.clone()
            },
            &rules,
        );
        assert_eq!(again.body, out.body, "not idempotent: {}", ctx());
        assert_eq!(again.title, out.title, "title not idempotent: {}", ctx());

        assert_eq!(
            out.blocked(),
            out.findings.iter().any(|f| f.mode == Mode::Block)
        );
    }
}

#[test]
fn empty_and_whitespace_messages_never_block_or_panic() {
    let mut cfg = Config::default();
    cfg.tabs.commit.subject_max = Some(RuleSetting::new(1, Mode::Block));
    cfg.tabs.commit.conventional = Some(RuleSetting::new(Conventional::Required, Mode::Block));
    let rules = effective(&cfg, Tab::Commit, None);
    for body in ["", " ", "\n\n\n", "\t", "\r\n"] {
        let out = process(
            &Input {
                body: Some(body),
                ..Default::default()
            },
            &rules,
        );
        assert!(!out.blocked(), "{body:?}");
    }
}
