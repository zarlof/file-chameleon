//! Job queue: runs jobs on worker threads with per-engine concurrency limits,
//! emits a [`JobSnapshot`] on every state/progress change, keeps the last 100 finished jobs.
//!
//! Workers are plain threads spawned on demand and gone once their pool's queue drains.
//! The listener is always called outside the queue's lock, so it may call back into the queue.

use super::{JobId, JobRequest, JobSnapshot, JobState, Operation};
use crate::engines::Executor;
use crate::formats::{self, Kind, planner, registry::EngineId};
use crate::{CancelToken, Error, now_ms};
use std::collections::{BTreeMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub type JobListener = Arc<dyn Fn(&JobSnapshot) + Send + Sync>;

const MAX_FINISHED: usize = 100;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pool {
    /// In-process engines: image, svg, data, pdf.
    Cpu = 0,
    /// ffmpeg and pdfium: one at a time.
    Single = 1,
}

/// Which pool a job runs in, from its first input's format and the operation.
fn pool_for(inputs: &[PathBuf], op: &Operation) -> Pool {
    let kind = inputs
        .first()
        .and_then(|p| formats::detect_path(p))
        .map(|f| (f.id, f.kind));
    match (op, kind) {
        (Operation::Convert { to }, Some((from, _))) => {
            match planner::plan(from, to).and_then(|r| r.steps.first().map(|s| s.engine)) {
                Some(EngineId::Ffmpeg | EngineId::Pdfium) => Pool::Single,
                _ => Pool::Cpu,
            }
        }
        (
            Operation::Compress { .. }
            | Operation::Trim { .. }
            | Operation::Mute
            | Operation::Speed { .. }
            | Operation::Crop { .. }
            | Operation::CropAspect { .. },
            Some((_, Kind::Video | Kind::Audio)),
        ) => Pool::Single,
        _ => Pool::Cpu,
    }
}

struct Entry {
    snap: JobSnapshot,
    options: super::ConvertOptions,
    cancel: CancelToken,
    pool: Pool,
    last_emit: Instant,
}

#[derive(Default)]
struct State {
    next_id: JobId,
    jobs: BTreeMap<JobId, Entry>,
    /// Waiting job ids per pool, FIFO.
    waiting: [VecDeque<JobId>; 2],
    /// Live worker threads per pool.
    workers: [usize; 2],
    /// Finished job ids, oldest first.
    finished: VecDeque<JobId>,
}

struct Inner {
    executor: Arc<dyn Executor>,
    listener: JobListener,
    limits: [usize; 2],
    state: Mutex<State>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock only means a listener/executor panicked elsewhere; the data is intact.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A panicking listener must not take a worker (and its slot accounting) down with it.
    fn emit(&self, snap: &JobSnapshot) {
        let _ = catch_unwind(AssertUnwindSafe(|| (self.listener)(snap)));
    }
}

pub struct JobQueue {
    inner: Arc<Inner>,
}

impl JobQueue {
    /// `executor` runs one job (route planning + engine calls); `listener` gets every update.
    /// Progress updates are throttled to at most ~10 per second per job.
    pub fn new(executor: Arc<dyn Executor>, listener: JobListener) -> Self {
        let cpu = std::thread::available_parallelism()
            .map(|n| n.get() / 2)
            .unwrap_or(1)
            .max(1);
        Self {
            inner: Arc::new(Inner {
                executor,
                listener,
                limits: [cpu, 1],
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// Split a request into jobs: `MergePdf` and `Zip` -> one job with all inputs; every other
    /// operation -> one job per input. Returns ids in input order.
    ///
    /// Operations that [`Operation::queueable`] rejects (`Annotate`, `CopyTo` without a folder)
    /// are never queued: the result is empty. Callers report the reason from `queueable`.
    pub fn submit(&self, req: JobRequest) -> Vec<JobId> {
        if req.inputs.is_empty() || req.op.queueable().is_err() {
            return Vec::new();
        }
        let groups: Vec<Vec<PathBuf>> = if matches!(req.op, Operation::MergePdf | Operation::Zip) {
            vec![req.inputs.clone()]
        } else {
            req.inputs.iter().map(|p| vec![p.clone()]).collect()
        };

        // Phase 1: register as Queued and announce, before any worker can see them.
        let mut ids = Vec::with_capacity(groups.len());
        let mut snaps = Vec::with_capacity(groups.len());
        {
            let mut st = self.inner.lock();
            for inputs in groups {
                st.next_id += 1;
                let id = st.next_id;
                let inputs_for_pool = inputs.clone();
                let snap = JobSnapshot {
                    id,
                    inputs,
                    op: req.op.clone(),
                    state: JobState::Queued,
                    progress: 0.0,
                    outputs: Vec::new(),
                    error_code: None,
                    error: None,
                    created_at: now_ms(),
                    finished_at: None,
                };
                snaps.push(snap.clone());
                st.jobs.insert(
                    id,
                    Entry {
                        snap,
                        options: req.options.clone(),
                        cancel: CancelToken::new(),
                        pool: pool_for(&inputs_for_pool, &req.op),
                        last_emit: Instant::now(),
                    },
                );
                ids.push(id);
            }
        }
        for s in &snaps {
            self.inner.emit(s);
        }

        // Phase 2: make them runnable and make sure enough workers exist.
        let to_spawn = {
            let mut st = self.inner.lock();
            for id in &ids {
                // Cancelled in between? Then it is no longer Queued.
                if let Some(e) = st.jobs.get(id)
                    && e.snap.state == JobState::Queued
                {
                    let p = e.pool as usize;
                    st.waiting[p].push_back(*id);
                }
            }
            // Busy workers count in `workers`, so only add as many as there are waiting
            // jobs and free slots.
            let mut spawn = [0usize; 2];
            for (p, slot) in spawn.iter_mut().enumerate() {
                let free = self.inner.limits[p].saturating_sub(st.workers[p]);
                *slot = st.waiting[p].len().min(free);
                st.workers[p] += *slot;
            }
            spawn
        };
        for (p, n) in to_spawn.into_iter().enumerate() {
            let pool = if p == 0 { Pool::Cpu } else { Pool::Single };
            for _ in 0..n {
                spawn_worker(&self.inner, pool);
            }
        }
        ids
    }

    /// Cancel a queued or running job. Returns false if unknown or already finished.
    pub fn cancel(&self, id: JobId) -> bool {
        let snap = {
            let mut st = self.inner.lock();
            let Some(entry) = st.jobs.get_mut(&id) else {
                return false;
            };
            match entry.snap.state {
                JobState::Running => {
                    entry.cancel.cancel();
                    return true;
                }
                JobState::Queued => {
                    entry.cancel.cancel();
                    entry.snap.state = JobState::Cancelled;
                    entry.snap.finished_at = Some(now_ms());
                    let snap = entry.snap.clone();
                    let pool = entry.pool as usize;
                    st.waiting[pool].retain(|j| *j != id);
                    st.finished.push_back(id);
                    trim_finished(&mut st);
                    snap
                }
                _ => return false,
            }
        };
        self.inner.emit(&snap);
        true
    }

    /// Active jobs first (queued/running, oldest first), then finished (newest first).
    pub fn snapshot(&self) -> Vec<JobSnapshot> {
        let st = self.inner.lock();
        let mut out: Vec<JobSnapshot> = st
            .jobs
            .values()
            .filter(|e| !e.snap.state.is_finished())
            .map(|e| e.snap.clone())
            .collect();
        out.extend(
            st.finished
                .iter()
                .rev()
                .filter_map(|id| st.jobs.get(id))
                .map(|e| e.snap.clone()),
        );
        out
    }

    /// Drop finished jobs from memory.
    pub fn clear_finished(&self) {
        let mut st = self.inner.lock();
        let ids: Vec<JobId> = st.finished.drain(..).collect();
        for id in ids {
            st.jobs.remove(&id);
        }
    }
}

fn trim_finished(st: &mut State) {
    while st.finished.len() > MAX_FINISHED {
        if let Some(old) = st.finished.pop_front() {
            st.jobs.remove(&old);
        }
    }
}

fn spawn_worker(inner: &Arc<Inner>, pool: Pool) {
    let worker_inner = Arc::clone(inner);
    let spawned = std::thread::Builder::new()
        .name("fc-job-worker".into())
        .spawn(move || worker_loop(&worker_inner, pool));
    if spawned.is_err() {
        // Could not start a thread: give the slot back; another worker (or a later submit) picks up.
        let mut st = inner.lock();
        st.workers[pool as usize] = st.workers[pool as usize].saturating_sub(1);
    }
}

fn worker_loop(inner: &Arc<Inner>, pool: Pool) {
    loop {
        // Take the next runnable job, or retire (decided under the same lock as submit's spawn count).
        let (id, inputs, op, options, cancel, running_snap) = {
            let mut st = inner.lock();
            let next = loop {
                match st.waiting[pool as usize].pop_front() {
                    None => break None,
                    Some(id) => {
                        if st
                            .jobs
                            .get(&id)
                            .is_some_and(|e| e.snap.state == JobState::Queued)
                        {
                            break Some(id);
                        }
                    }
                }
            };
            let Some(id) = next else {
                st.workers[pool as usize] -= 1;
                return;
            };
            let Some(e) = st.jobs.get_mut(&id) else {
                continue;
            };
            e.snap.state = JobState::Running;
            e.last_emit = Instant::now();
            (
                id,
                e.snap.inputs.clone(),
                e.snap.op.clone(),
                e.options.clone(),
                e.cancel.clone(),
                e.snap.clone(),
            )
        };
        inner.emit(&running_snap);

        let progress = |p: f32| {
            let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 1.0) };
            let snap = {
                let mut st = inner.lock();
                let Some(e) = st.jobs.get_mut(&id) else {
                    return;
                };
                e.snap.progress = p;
                if p < 1.0 && e.last_emit.elapsed() < PROGRESS_INTERVAL {
                    return;
                }
                e.last_emit = Instant::now();
                e.snap.clone()
            };
            inner.emit(&snap);
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            inner
                .executor
                .run(&inputs, &op, &options, &cancel, &progress)
        }))
        .unwrap_or_else(|_| Err(Error::Failed("internal error".into())));

        let snap = {
            let mut st = inner.lock();
            let Some(e) = st.jobs.get_mut(&id) else {
                continue;
            };
            e.snap.finished_at = Some(now_ms());
            match result {
                Ok(outputs) => {
                    e.snap.state = JobState::Done;
                    e.snap.progress = 1.0;
                    e.snap.outputs = outputs;
                }
                Err(Error::Cancelled) => e.snap.state = JobState::Cancelled,
                Err(err) => {
                    e.snap.state = JobState::Failed;
                    e.snap.error_code = Some(err.code().to_string());
                    e.snap.error = Some(err.to_string());
                }
            }
            let snap = e.snap.clone();
            st.finished.push_back(id);
            trim_finished(&mut st);
            snap
        };
        inner.emit(&snap);
    }
}
