//! Job queue: runs jobs on worker threads with per-engine concurrency limits,
//! emits a [`JobSnapshot`] on every state/progress change, keeps the last 100 finished jobs.
//! OWNER: Task A. Replace the `todo!()` bodies; keep the signatures.

use super::{JobId, JobRequest, JobSnapshot};
use crate::engines::Executor;
use std::sync::Arc;

pub type JobListener = Arc<dyn Fn(&JobSnapshot) + Send + Sync>;

pub struct JobQueue {
    _private: (),
}

impl JobQueue {
    /// `executor` runs one job (route planning + engine calls); `listener` gets every update.
    /// Progress updates are throttled to at most ~10 per second per job.
    pub fn new(executor: Arc<dyn Executor>, listener: JobListener) -> Self {
        let _ = (executor, listener);
        todo!("Task A")
    }

    /// Split a request into jobs: `Convert`/`Compress`/`Trim`/`Mute`/`Speed`/`Crop`/`SplitPdf`
    /// -> one job per input; `MergePdf` -> one job with all inputs. Returns ids in input order.
    pub fn submit(&self, req: JobRequest) -> Vec<JobId> {
        let _ = req;
        todo!("Task A")
    }

    /// Cancel a queued or running job. Returns false if unknown or already finished.
    pub fn cancel(&self, id: JobId) -> bool {
        let _ = id;
        todo!("Task A")
    }

    /// Active jobs first (queued/running, oldest first), then finished (newest first).
    pub fn snapshot(&self) -> Vec<JobSnapshot> {
        todo!("Task A")
    }

    /// Drop finished jobs from memory.
    pub fn clear_finished(&self) {
        todo!("Task A")
    }
}
