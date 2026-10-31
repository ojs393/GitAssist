//! Background jobs (publish, fix-all, apply) with persisted progress.
//! Survives UI navigation; Cancel or app quit stops work. Resume skips done steps.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::publish::{self, Selection};
use crate::secure;
use crate::targets;

const MAX_JOBS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Publish,
    FixAll,
    ApplyAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub id: String,
    pub label: String,
    pub status: StepStatus,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub started_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub which: String,
    #[serde(default)]
    pub adapter_id: Option<String>,
    pub steps: Vec<JobStep>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    jobs: Vec<Job>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn jobs_path(paths: &Paths) -> PathBuf {
    paths.data_dir.join("jobs.json")
}

fn load_store(paths: &Paths) -> Store {
    let Ok(s) = fs::read_to_string(jobs_path(paths)) else {
        return Store::default();
    };
    serde_json::from_str(&s).unwrap_or_default()
}

fn save_store(paths: &Paths, store: &Store) -> Result<()> {
    fs::create_dir_all(&paths.data_dir)?;
    secure::lock_dir(&paths.data_dir);
    let path = jobs_path(paths);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(store)?)?;
    fs::rename(&tmp, &path)?;
    secure::lock_file(&path);
    Ok(())
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering as O};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    format!("j{}-{}", now(), SEQ.fetch_add(1, O::Relaxed))
}

/// In-memory cancel flags keyed by job id.
fn cancels() -> &'static Mutex<std::collections::HashMap<String, Arc<AtomicBool>>> {
    static M: OnceLock<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn cancel_flag(id: &str) -> Arc<AtomicBool> {
    let mut m = cancels().lock().unwrap_or_else(|e| e.into_inner());
    m.entry(id.to_string())
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn drop_cancel(id: &str) {
    if let Ok(mut m) = cancels().lock() {
        m.remove(id);
    }
}

pub fn list(paths: &Paths) -> Vec<Job> {
    let mut jobs = load_store(paths).jobs;
    jobs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    jobs
}

pub fn get(paths: &Paths, id: &str) -> Option<Job> {
    load_store(paths).jobs.into_iter().find(|j| j.id == id)
}

fn upsert(paths: &Paths, job: Job) -> Result<Job> {
    let mut store = load_store(paths);
    if let Some(i) = store.jobs.iter().position(|j| j.id == job.id) {
        store.jobs[i] = job.clone();
    } else {
        store.jobs.insert(0, job.clone());
    }
    if store.jobs.len() > MAX_JOBS {
        store.jobs.truncate(MAX_JOBS);
    }
    save_store(paths, &store)?;
    Ok(job)
}

/// Mark any still-running jobs as paused (call on app quit / startup recovery).
pub fn pause_running(paths: &Paths) -> Result<()> {
    let mut store = load_store(paths);
    let t = now();
    let mut changed = false;
    for j in &mut store.jobs {
        if j.status == JobStatus::Running {
            j.status = JobStatus::Paused;
            j.updated_at = t;
            for s in &mut j.steps {
                if s.status == StepStatus::Running {
                    s.status = StepStatus::Pending;
                    s.message = "paused".into();
                }
            }
            changed = true;
        }
    }
    if changed {
        save_store(paths, &store)?;
    }
    // Clear in-memory cancel flags
    if let Ok(mut m) = cancels().lock() {
        m.clear();
    }
    Ok(())
}

pub fn request_cancel(paths: &Paths, id: &str) -> Result<Job> {
    let flag = cancel_flag(id);
    flag.store(true, Ordering::SeqCst);
    let mut job = get(paths, id).context("job not found")?;
    if matches!(job.status, JobStatus::Running | JobStatus::Paused) {
        job.status = JobStatus::Cancelled;
        job.updated_at = now();
        job.error = Some("cancelled".into());
        for s in &mut job.steps {
            if matches!(s.status, StepStatus::Pending | StepStatus::Running) {
                s.status = StepStatus::Skipped;
                s.message = "cancelled".into();
            }
        }
        upsert(paths, job.clone())?;
    }
    Ok(job)
}

fn update_step(job: &mut Job, step_id: &str, status: StepStatus, message: impl Into<String>) {
    if let Some(s) = job.steps.iter_mut().find(|s| s.id == step_id) {
        s.status = status;
        s.message = message.into();
    }
    job.updated_at = now();
}

/// Start a publish job. `which` is a label or `--all`.
pub fn start_publish(paths: &Paths, repo: &Path, which: &str) -> Result<Job> {
    let data = targets::load(paths, repo);
    let which = which.trim();
    let chosen: Vec<&targets::PublishTarget> = if which == "--all" || which == "all" {
        targets::for_push_all(&data)
    } else {
        let t = targets::find_by_label(&data, which)
            .with_context(|| format!("no enabled target labeled `{which}`"))?;
        vec![t]
    };
    if chosen.is_empty() {
        bail!("no targets to publish");
    }
    let t = now();
    let id = new_id();
    let job = Job {
        id: id.clone(),
        kind: JobKind::Publish,
        status: JobStatus::Running,
        started_at: t,
        updated_at: t,
        repo: Some(repo.to_string_lossy().into_owned()),
        label: if which == "--all" || which == "all" {
            "Push all".into()
        } else {
            format!("Push {which}")
        },
        which: which.to_string(),
        adapter_id: None,
        steps: chosen
            .iter()
            .map(|tg| JobStep {
                id: tg.label.clone(),
                label: tg.label.clone(),
                status: StepStatus::Pending,
                message: String::new(),
            })
            .collect(),
        error: None,
    };
    cancel_flag(&id).store(false, Ordering::SeqCst);
    upsert(paths, job.clone())?;
    Ok(job)
}

pub fn start_fix_all(paths: &Paths, step_labels: Vec<(String, String)>) -> Result<Job> {
    let t = now();
    let id = new_id();
    let job = Job {
        id: id.clone(),
        kind: JobKind::FixAll,
        status: JobStatus::Running,
        started_at: t,
        updated_at: t,
        repo: None,
        label: "Fix all".into(),
        which: String::new(),
        adapter_id: None,
        steps: step_labels
            .into_iter()
            .map(|(id, label)| JobStep {
                id,
                label,
                status: StepStatus::Pending,
                message: String::new(),
            })
            .collect(),
        error: None,
    };
    cancel_flag(&id).store(false, Ordering::SeqCst);
    upsert(paths, job.clone())?;
    Ok(job)
}

pub fn start_apply_adapter(paths: &Paths, adapter_id: &str, name: &str) -> Result<Job> {
    let t = now();
    let id = new_id();
    let job = Job {
        id: id.clone(),
        kind: JobKind::ApplyAdapter,
        status: JobStatus::Running,
        started_at: t,
        updated_at: t,
        repo: None,
        label: format!("Apply {name}"),
        which: String::new(),
        adapter_id: Some(adapter_id.into()),
        steps: vec![JobStep {
            id: adapter_id.into(),
            label: name.into(),
            status: StepStatus::Pending,
            message: String::new(),
        }],
        error: None,
    };
    cancel_flag(&id).store(false, Ordering::SeqCst);
    upsert(paths, job.clone())?;
    Ok(job)
}

/// Run or resume a publish job. Skips steps already Done.
pub fn run_publish(paths: &Paths, cfg: &gitassist_core::Config, job_id: &str) -> Result<Job> {
    let mut job = get(paths, job_id).context("job not found")?;
    if job.kind != JobKind::Publish {
        bail!("not a publish job");
    }
    let repo = PathBuf::from(job.repo.as_deref().context("publish job missing repo")?);
    let flag = cancel_flag(job_id);
    flag.store(false, Ordering::SeqCst);
    job.status = JobStatus::Running;
    job.error = None;
    job.updated_at = now();
    upsert(paths, job.clone())?;

    for i in 0..job.steps.len() {
        if flag.load(Ordering::SeqCst) {
            job.status = JobStatus::Cancelled;
            job.error = Some("cancelled".into());
            for s in &mut job.steps {
                if matches!(s.status, StepStatus::Pending | StepStatus::Running) {
                    s.status = StepStatus::Skipped;
                    s.message = "cancelled".into();
                }
            }
            upsert(paths, job.clone())?;
            drop_cancel(job_id);
            return Ok(job);
        }
        if job.steps[i].status == StepStatus::Done {
            continue;
        }
        let label = job.steps[i].id.clone();
        update_step(&mut job, &label, StepStatus::Running, "pushing…");
        upsert(paths, job.clone())?;

        match publish::publish(paths, &repo, cfg, Selection::Label(&label)) {
            Ok(report) => {
                let r = report.results.first();
                let ok = report.ok && r.map(|x| x.ok).unwrap_or(false);
                let msg = r.map(|x| x.message.clone()).unwrap_or_default();
                if ok {
                    update_step(&mut job, &label, StepStatus::Done, msg);
                } else {
                    update_step(&mut job, &label, StepStatus::Failed, msg);
                    job.status = JobStatus::Failed;
                    job.error = Some(format!("{label} failed"));
                    upsert(paths, job.clone())?;
                    drop_cancel(job_id);
                    return Ok(job);
                }
            }
            Err(e) => {
                update_step(&mut job, &label, StepStatus::Failed, e.to_string());
                job.status = JobStatus::Failed;
                job.error = Some(e.to_string());
                upsert(paths, job.clone())?;
                drop_cancel(job_id);
                return Ok(job);
            }
        }
        upsert(paths, job.clone())?;
    }

    job.status = JobStatus::Done;
    job.updated_at = now();
    upsert(paths, job.clone())?;
    drop_cancel(job_id);
    Ok(job)
}

/// Mark a Fix-all / Apply step and persist. Callers drive the actual work.
pub fn set_step(
    paths: &Paths,
    job_id: &str,
    step_id: &str,
    status: StepStatus,
    message: impl Into<String>,
) -> Result<Job> {
    let mut job = get(paths, job_id).context("job not found")?;
    update_step(&mut job, step_id, status, message);
    if status == StepStatus::Failed {
        job.status = JobStatus::Failed;
    }
    upsert(paths, job)
}

pub fn finish_job(paths: &Paths, job_id: &str, status: JobStatus, error: Option<String>) -> Result<Job> {
    let mut job = get(paths, job_id).context("job not found")?;
    job.status = status;
    job.error = error;
    job.updated_at = now();
    if matches!(status, JobStatus::Done | JobStatus::Cancelled | JobStatus::Failed) {
        for s in &mut job.steps {
            if s.status == StepStatus::Running {
                s.status = StepStatus::Pending;
            }
        }
    }
    upsert(paths, job.clone())?;
    drop_cancel(job_id);
    Ok(job)
}

pub fn is_cancelled(job_id: &str) -> bool {
    cancels()
        .lock()
        .ok()
        .and_then(|m| m.get(job_id).map(|f| f.load(Ordering::SeqCst)))
        .unwrap_or(false)
}

pub fn resume(paths: &Paths, cfg: &gitassist_core::Config, job_id: &str) -> Result<Job> {
    let job = get(paths, job_id).context("job not found")?;
    match job.kind {
        JobKind::Publish => {
            if !matches!(job.status, JobStatus::Paused | JobStatus::Cancelled | JobStatus::Failed) {
                if job.status == JobStatus::Done {
                    return Ok(job);
                }
                if job.status == JobStatus::Running {
                    bail!("job already running");
                }
            }
            let mut job = job;
            for s in &mut job.steps {
                if matches!(s.status, StepStatus::Failed | StepStatus::Skipped) {
                    s.status = StepStatus::Pending;
                    s.message.clear();
                }
            }
            job.status = JobStatus::Running;
            upsert(paths, job)?;
            run_publish(paths, cfg, job_id)
        }
        JobKind::FixAll | JobKind::ApplyAdapter => {
            bail!("resume this job from the Overview Fix all / Apply action");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn paths() -> (tempfile::TempDir, Paths) {
        let d = tempdir().unwrap();
        let p = Paths::from_dirs(d.path().join("c"), d.path().join("d"));
        fs::create_dir_all(&p.data_dir).unwrap();
        (d, p)
    }

    #[test]
    fn persist_and_skip_done_on_resume_shape() {
        let (_d, p) = paths();
        let t = now();
        let job = Job {
            id: "j1".into(),
            kind: JobKind::Publish,
            status: JobStatus::Paused,
            started_at: t,
            updated_at: t,
            repo: Some("/tmp/r".into()),
            label: "Push all".into(),
            which: "--all".into(),
            adapter_id: None,
            steps: vec![
                JobStep {
                    id: "personal".into(),
                    label: "personal".into(),
                    status: StepStatus::Done,
                    message: "ok".into(),
                },
                JobStep {
                    id: "public".into(),
                    label: "public".into(),
                    status: StepStatus::Pending,
                    message: String::new(),
                },
            ],
            error: None,
        };
        upsert(&p, job).unwrap();
        let loaded = get(&p, "j1").unwrap();
        assert_eq!(loaded.steps[0].status, StepStatus::Done);
        assert_eq!(loaded.steps[1].status, StepStatus::Pending);
        pause_running(&p).unwrap();
    }
}
