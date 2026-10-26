use regex::{Regex, RegexBuilder};

use crate::config::{Config, Conventional, Mode, RuleSetting, Style, TabConfig};
use crate::tab::Tab;
use crate::template;

/// The fully-resolved rule set for one tab: built-in defaults, then global
/// settings, then the user's tab overrides, then (if allowed) project overrides.
#[derive(Debug, Clone)]
pub struct EffectiveRules {
    pub tab: Tab,
    pub enabled: bool,
    pub subject_max: Option<RuleSetting<usize>>,
    pub body_max: Option<RuleSetting<usize>>,
    pub total_max: Option<RuleSetting<usize>>,
    pub wrap: Option<RuleSetting<usize>>,
    pub conventional: RuleSetting<Conventional>,
    pub template: String,
    pub required_sections: Vec<String>,
    pub sections_mode: Mode,
    pub managed_table: bool,
    pub banned_phrases: RuleSetting<Vec<String>>,
    pub growth_max: Option<RuleSetting<usize>>,
    pub strip_trailers: bool,
    pub strip_patterns: Vec<Regex>,
    pub strip_mode: Mode,
    pub add_trailers: Vec<String>,
    pub style: Style,
}

/// Defaults never block: every limit starts as a warning so nothing breaks
/// on first install.
pub fn builtin(tab: Tab) -> TabConfig {
    let warn = |n| Some(RuleSetting::new(n, Mode::Warn));
    let mut t = TabConfig {
        enabled: Some(true),
        conventional: Some(RuleSetting::new(Conventional::Off, Mode::Warn)),
        template: Some(String::new()),
        sections_mode: Some(Mode::Warn),
        managed_table: Some(false),
        banned_phrases: Some(RuleSetting::new(Vec::new(), Mode::Warn)),
        strip_trailers: Some(true),
        ..Default::default()
    };
    match tab {
        Tab::Commit | Tab::DocsCommit => {
            t.subject_max = warn(72);
            t.body_max = warn(5000);
            t.wrap = Some(RuleSetting::new(72, Mode::Fix));
        }
        Tab::Merge => {
            t.subject_max = warn(100);
            t.body_max = warn(5000);
            t.wrap = Some(RuleSetting::new(72, Mode::Fix));
        }
        Tab::PrCreate => {
            t.subject_max = warn(72);
            t.body_max = warn(10_000);
        }
        Tab::PrUpdate => {
            t.subject_max = warn(72);
            t.body_max = warn(10_000);
            t.growth_max = warn(3000);
        }
        Tab::PrComment | Tab::Review => {
            t.body_max = warn(5000);
        }
        Tab::Issue => {
            t.subject_max = warn(100);
            t.body_max = warn(10_000);
        }
    }
    t
}

fn overlay(base: &mut TabConfig, over: &TabConfig) {
    macro_rules! take {
        ($($f:ident),*) => { $( if over.$f.is_some() { base.$f = over.$f.clone(); } )* };
    }
    take!(
        enabled,
        subject_max,
        body_max,
        total_max,
        wrap,
        conventional,
        template,
        sections_mode,
        managed_table,
        banned_phrases,
        growth_max,
        strip_trailers,
        add_trailers,
        style
    );
}

/// Fields a repository's `.gitassist.toml` is never allowed to change.
fn sanitize_project(project: &TabConfig) -> TabConfig {
    TabConfig {
        enabled: None,
        strip_trailers: None,
        add_trailers: None,
        ..project.clone()
    }
}

pub fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|p| {
            RegexBuilder::new(p)
                .case_insensitive(true)
                .multi_line(true)
                .build()
                .ok()
        })
        .collect()
}

pub fn effective(cfg: &Config, tab: Tab, project: Option<&TabConfig>) -> EffectiveRules {
    let mut t = builtin(tab);
    overlay(&mut t, cfg.tabs.get(tab));
    if cfg.allow_project_overrides {
        if let Some(p) = project {
            overlay(&mut t, &sanitize_project(p));
        }
    }

    // Rules that do not apply to a tab are dropped regardless of what the file says.
    if !tab.has_title() {
        t.subject_max = None;
    }
    if tab != Tab::PrUpdate {
        t.growth_max = None;
    }
    let conventional = if matches!(tab, Tab::PrComment | Tab::Review | Tab::Issue) {
        RuleSetting::new(Conventional::Off, Mode::Warn)
    } else {
        t.conventional
            .unwrap_or(RuleSetting::new(Conventional::Off, Mode::Warn))
    };
    let wrap = t.wrap.filter(|w| w.value > 0);
    let template = t.template.unwrap_or_default();

    EffectiveRules {
        tab,
        enabled: cfg.enabled && t.enabled.unwrap_or(true),
        subject_max: t.subject_max.filter(|r| r.value > 0),
        body_max: t.body_max.filter(|r| r.value > 0),
        total_max: t.total_max.filter(|r| r.value > 0),
        wrap,
        conventional,
        required_sections: template::required_sections(&template),
        template,
        sections_mode: t.sections_mode.unwrap_or(Mode::Warn),
        managed_table: t.managed_table.unwrap_or(false)
            && matches!(tab, Tab::PrCreate | Tab::PrUpdate),
        banned_phrases: t
            .banned_phrases
            .unwrap_or(RuleSetting::new(Vec::new(), Mode::Warn)),
        growth_max: t.growth_max.filter(|r| r.value > 0),
        strip_trailers: t.strip_trailers.unwrap_or(true),
        strip_patterns: compile_patterns(&cfg.global.trailers.strip_patterns),
        strip_mode: cfg.global.trailers.strip_mode,
        // Global trailers are commit trailers; GitHub text only gets trailers its tab asks for.
        add_trailers: t.add_trailers.unwrap_or_else(|| {
            if tab.is_commit_message() {
                cfg.global.trailers.add.clone()
            } else {
                Vec::new()
            }
        }),
        style: t.style.unwrap_or_else(|| cfg.global.style.clone()),
    }
}
