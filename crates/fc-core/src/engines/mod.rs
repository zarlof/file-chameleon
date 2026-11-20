//! Conversion engines and the job executor that chains them along a planned route.
//!
//! Ownership: `mod.rs` (DefaultExecutor) = Task A; `image.rs`, `svg.rs`, `pdf.rs`,
//! `data.rs` = Task B; `ffmpeg.rs` = Task C. Engine functions are plain functions
//! with fixed signatures (see each file) so tasks can be built in parallel.
//!
//! Contract for every engine function:
//! - Read only from the given inputs; write only to the given `output` path
//!   (the executor hands a temp path and commits it afterwards).
//! - Check `cancel` between units of work; return `Err(Error::Cancelled)` promptly.
//! - Report progress in 0.0..=1.0 through `progress` when meaningful.
//! - Never panic on malformed input; return `Error::Failed`/`Unsupported`.

pub mod data;
pub mod ffmpeg;
pub mod image;
pub mod pdf;
pub mod svg;

use crate::jobs::{ConvertOptions, Operation};
use crate::{CancelToken, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Finds external tool files. Implemented by `engine_mgr::EngineManager`.
/// Queried per job so a tool installed while the app runs is picked up.
pub trait ToolLocator: Send + Sync {
    /// `tool` is a manifest id ("ffmpeg", "pdfium"); `file` a file it provides
    /// ("ffmpeg.exe", "ffprobe.exe", "pdfium.dll").
    fn locate(&self, tool: &str, file: &str) -> Option<PathBuf>;
}

/// Runs one job to completion and returns the committed output paths.
pub trait Executor: Send + Sync {
    fn run(
        &self,
        inputs: &[PathBuf],
        op: &Operation,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>>;
}

/// The real executor.
/// - `Convert`: detect the input format, `formats::planner::plan` a route, run each step
///   into a temp file (intermediates in a temp dir removed afterwards), name the final file
///   with `jobs::naming::output_path`, write to `naming::temp_path`, then `naming::commit`.
///   Overall progress = step index + step progress, divided by step count.
///   pdf -> raster renders every page: 1 page -> one file; N pages -> a new folder named
///   after the input (unique) holding `<stem>-<n>.<ext>`.
/// - `MergePdf` -> `pdf::merge`; output `<first stem> (merged).pdf`.
/// - `SplitPdf` -> `pdf::split` into a unique folder named after the input.
/// - `Compress` -> by input kind: image -> `image::compress`, video/audio -> `ffmpeg`
///   `compress`, pdf -> `pdf::compress`; suffix " (compressed)".
/// - `Trim`/`Mute`/`Speed` -> ffmpeg; suffixes " (trimmed)", " (muted)", " (speed)".
/// - `Crop` -> image or ffmpeg by kind; suffix " (cropped)".
/// - Any panic inside an engine is caught (`catch_unwind`) and reported as `Error::Failed`.
/// - On error or cancel, temp/partial files are removed.
pub struct DefaultExecutor {
    pub tools: Arc<dyn ToolLocator>,
}

impl Executor for DefaultExecutor {
    fn run(
        &self,
        inputs: &[PathBuf],
        op: &Operation,
        options: &ConvertOptions,
        cancel: &CancelToken,
        progress: &dyn Fn(f32),
    ) -> Result<Vec<PathBuf>> {
        let job = run::Job {
            tools: self.tools.as_ref(),
            options,
            cancel,
        };
        let mut guard = run::Guard::default();
        let result = job.run(inputs, op, progress, &mut guard);
        if result.is_ok() {
            guard.keep_outputs();
        }
        result
    }
}

/// Helper used by engines: resolve the output encoding for raster images.
pub(crate) fn quality_or(options: &ConvertOptions, default: u8) -> u8 {
    options.quality.unwrap_or(default).clamp(1, 100)
}

/// Convenience used in tests and the executor.
pub fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

/// Executor internals: routing, temp files, cleanup.
mod run {
    use super::{ConvertOptions, Operation, ToolLocator, data, ffmpeg::Ffmpeg, image, pdf, svg};
    use crate::formats::{self, Kind, planner, registry::EngineId};
    use crate::jobs::naming;
    use crate::{CancelToken, Error, Result};
    use std::fs::{self, OpenOptions};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Removes everything a failed or cancelled job left behind. Dropped last, so it also
    /// covers early returns and panics in the executor itself.
    #[derive(Default)]
    pub struct Guard {
        temps: Vec<PathBuf>,
        outputs: Vec<PathBuf>,
        dirs: Vec<PathBuf>,
        work_dir: Option<PathBuf>,
        keep: bool,
    }

    impl Guard {
        pub fn keep_outputs(&mut self) {
            self.keep = true;
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            for t in &self.temps {
                let _ = fs::remove_file(t);
            }
            if let Some(w) = &self.work_dir {
                let _ = fs::remove_dir_all(w);
            }
            if !self.keep {
                for o in &self.outputs {
                    let _ = fs::remove_file(o);
                }
                for d in &self.dirs {
                    let _ = fs::remove_dir_all(d);
                }
            }
        }
    }

    pub struct Job<'a> {
        pub tools: &'a dyn ToolLocator,
        pub options: &'a ConvertOptions,
        pub cancel: &'a CancelToken,
    }

    fn missing(tool: &str) -> Error {
        Error::MissingTool(tool.to_string())
    }

    /// Run an engine call, turning a panic into `Error::Failed`.
    fn guarded<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
        catch_unwind(AssertUnwindSafe(f))
            .unwrap_or_else(|_| Err(Error::Failed("engine crashed".to_string())))
    }

    fn unique_work_name() -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!(
            "{:x}-{:x}-{:x}",
            crate::now_ms(),
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// Atomically claim a temp file next to `final_path` (`naming::temp_path`, or a numbered
    /// variant when another job or a crash left that name behind).
    fn claim_temp(final_path: &Path, g: &mut Guard) -> Result<PathBuf> {
        let base = naming::temp_path(final_path);
        let mut n = 0u32;
        loop {
            let candidate = if n == 0 {
                base.clone()
            } else {
                let stem = base
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let name = match base.extension() {
                    Some(e) => format!("{stem}{n}.{}", e.to_string_lossy()),
                    None => format!("{stem}{n}"),
                };
                base.with_file_name(name)
            };
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(_) => {
                    g.temps.push(candidate.clone());
                    return Ok(candidate);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn stem_of(input: &Path) -> String {
        input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".to_string())
    }

    impl Job<'_> {
        /// The path outputs are named after: `input`, or `<output_name>.<input ext>` beside it.
        fn naming_input(&self, input: &Path) -> PathBuf {
            let name = self
                .options
                .output_name
                .as_deref()
                .map(str::trim)
                .and_then(|n| Path::new(n).file_name())
                .filter(|n| !n.is_empty());
            match name {
                Some(n) => {
                    let mut file = n.to_os_string();
                    if let Some(ext) = input.extension() {
                        file.push(".");
                        file.push(ext);
                    }
                    input.with_file_name(file)
                }
                None => input.to_path_buf(),
            }
        }

        fn check_cancel(&self) -> Result<()> {
            if self.cancel.is_cancelled() {
                Err(Error::Cancelled)
            } else {
                Ok(())
            }
        }

        fn ffmpeg(&self) -> Result<Ffmpeg> {
            let ffmpeg = self
                .tools
                .locate("ffmpeg", "ffmpeg.exe")
                .ok_or_else(|| missing("ffmpeg"))?;
            let ffprobe = self.tools.locate("ffmpeg", "ffprobe.exe");
            Ok(Ffmpeg { ffmpeg, ffprobe })
        }

        fn pdfium(&self) -> Result<PathBuf> {
            self.tools
                .locate("pdfium", "pdfium.dll")
                .ok_or_else(|| missing("pdfium"))
        }

        fn work_dir(&self, g: &mut Guard) -> Result<PathBuf> {
            if let Some(w) = &g.work_dir {
                return Ok(w.clone());
            }
            let dir = std::env::temp_dir()
                .join("file-chameleon")
                .join(unique_work_name());
            fs::create_dir_all(&dir)?;
            g.work_dir = Some(dir.clone());
            Ok(dir)
        }

        /// Write via a temp file and commit it to `final_path`; returns the committed path.
        fn produce(
            &self,
            g: &mut Guard,
            final_path: &Path,
            write: impl FnOnce(&Path) -> Result<()>,
        ) -> Result<PathBuf> {
            self.check_cancel()?;
            let tmp = claim_temp(final_path, g)?;
            guarded(|| write(&tmp))?;
            self.check_cancel()?;
            let done = naming::commit(&tmp, final_path)?;
            g.outputs.push(done.clone());
            Ok(done)
        }

        pub fn run(
            &self,
            inputs: &[PathBuf],
            op: &Operation,
            progress: &dyn Fn(f32),
            g: &mut Guard,
        ) -> Result<Vec<PathBuf>> {
            if inputs.is_empty() {
                return Err(Error::Failed("no input files".to_string()));
            }
            for p in inputs {
                if !p.is_file() {
                    return Err(Error::NotFound(p.clone()));
                }
            }
            if let Operation::MergePdf = op {
                let out_dir = self.options.output_dir.as_deref();
                let base = self.naming_input(&inputs[0]);
                let out = naming::output_path(&base, "pdf", out_dir, Some(" (merged)"));
                let cancel = self.cancel;
                let done =
                    self.produce(g, &out, |tmp| pdf::merge(inputs, tmp, cancel, progress))?;
                return Ok(vec![done]);
            }
            let n = inputs.len() as f32;
            let mut all = Vec::new();
            for (i, input) in inputs.iter().enumerate() {
                let sub = |p: f32| progress((i as f32 + p) / n);
                let mut outs = match op {
                    Operation::Convert { to } => self.convert_one(input, to, &sub, g)?,
                    Operation::SplitPdf => self.split_one(input, &sub, g)?,
                    other => vec![self.tool_one(input, other, &sub, g)?],
                };
                all.append(&mut outs);
            }
            Ok(all)
        }

        /// `n == 1` -> one file via naming; `n > 1` -> new unique folder named after the input.
        fn final_paths(
            &self,
            input: &Path,
            ext: &str,
            n: usize,
            g: &mut Guard,
        ) -> Result<Vec<PathBuf>> {
            let out_dir = self.options.output_dir.as_deref();
            let base = self.naming_input(input);
            if n == 1 {
                return Ok(vec![naming::output_path(&base, ext, out_dir, None)]);
            }
            let stem = stem_of(&base);
            let dir = naming::unique_dir(&naming::dir_for(&base, out_dir), &stem)?;
            g.dirs.push(dir.clone());
            Ok((1..=n)
                .map(|k| dir.join(format!("{stem}-{k}.{ext}")))
                .collect())
        }

        fn convert_one(
            &self,
            input: &Path,
            to: &str,
            progress: &dyn Fn(f32),
            g: &mut Guard,
        ) -> Result<Vec<PathBuf>> {
            let from = formats::detect_path(input).ok_or_else(|| {
                Error::Unsupported(format!("unknown file type: {}", input.display()))
            })?;
            let route = planner::plan(from.id, to)
                .ok_or_else(|| Error::Unsupported(format!("{} -> {to}", from.id)))?;
            let out_ext = formats::output_ext(to)
                .ok_or_else(|| Error::Unsupported(format!("unknown target: {to}")))?;

            // Resolve tools before touching the disk.
            let mut ff = None;
            let mut pdfium_dll = None;
            for step in &route.steps {
                match step.engine {
                    EngineId::Ffmpeg if ff.is_none() => ff = Some(self.ffmpeg()?),
                    EngineId::Pdfium if pdfium_dll.is_none() => pdfium_dll = Some(self.pdfium()?),
                    _ => {}
                }
            }

            let cancel = self.cancel;
            let options = self.options;
            let steps = route.steps.len();
            let mut files = vec![input.to_path_buf()];
            for (i, step) in route.steps.iter().enumerate() {
                self.check_cancel()?;
                let last = i == steps - 1;
                let step_progress = |p: f32| progress((i as f32 + p) / steps as f32);
                let step_ext = formats::output_ext(step.to).unwrap_or(step.to);

                if step.engine == EngineId::Pdfium {
                    let dll = pdfium_dll.as_deref().ok_or_else(|| missing("pdfium"))?;
                    let src = files[0].clone();
                    let pages = guarded(|| pdf::page_count(&src))?;
                    if pages == 0 {
                        return Err(Error::Failed("pdf has no pages".to_string()));
                    }
                    if last {
                        let finals = self.final_paths(input, out_ext, pages, g)?;
                        let mut temps = Vec::with_capacity(pages);
                        for f in &finals {
                            temps.push(claim_temp(f, g)?);
                        }
                        guarded(|| {
                            pdf::render_pages(
                                dll,
                                &src,
                                step.to,
                                &temps,
                                options,
                                cancel,
                                &step_progress,
                            )
                        })?;
                        self.check_cancel()?;
                        let mut done = Vec::with_capacity(pages);
                        for (t, f) in temps.iter().zip(&finals) {
                            let p = naming::commit(t, f)?;
                            g.outputs.push(p.clone());
                            done.push(p);
                        }
                        return Ok(done);
                    }
                    let work = self.work_dir(g)?;
                    let mids: Vec<PathBuf> = (1..=pages)
                        .map(|k| work.join(format!("s{i}-{k}.{step_ext}")))
                        .collect();
                    guarded(|| {
                        pdf::render_pages(
                            dll,
                            &src,
                            step.to,
                            &mids,
                            options,
                            cancel,
                            &step_progress,
                        )
                    })?;
                    files = mids;
                    continue;
                }

                let count = files.len();
                let targets: Vec<PathBuf> = if last {
                    self.final_paths(input, out_ext, count, g)?
                } else {
                    let work = self.work_dir(g)?;
                    (0..count)
                        .map(|k| work.join(format!("s{i}-{k}.{step_ext}")))
                        .collect()
                };
                let mut next = Vec::with_capacity(count);
                for (k, (src, target)) in files.iter().zip(&targets).enumerate() {
                    let file_progress = |p: f32| step_progress((k as f32 + p) / count as f32);
                    let run_engine = |tmp: &Path| -> Result<()> {
                        match step.engine {
                            EngineId::Image => image::convert(src, step.to, tmp, options, cancel),
                            EngineId::Svg => svg::rasterize(src, step.to, tmp, options, cancel),
                            EngineId::Data => data::convert(src, step.from, step.to, tmp),
                            EngineId::Pdf => pdf::images_to_pdf(
                                std::slice::from_ref(src),
                                tmp,
                                cancel,
                                &file_progress,
                            ),
                            EngineId::Ffmpeg => ff
                                .as_ref()
                                .ok_or_else(|| missing("ffmpeg"))?
                                .convert(src, step.to, tmp, options, cancel, &file_progress),
                            EngineId::Pdfium => {
                                Err(Error::Failed("unexpected pdfium step".to_string()))
                            }
                        }
                    };
                    if last {
                        next.push(self.produce(g, target, run_engine)?);
                    } else {
                        // Intermediates live in the work dir: no commit, no naming.
                        guarded(|| run_engine(target))?;
                        next.push(target.clone());
                    }
                }
                files = next;
            }
            Ok(files)
        }

        fn split_one(
            &self,
            input: &Path,
            progress: &dyn Fn(f32),
            g: &mut Guard,
        ) -> Result<Vec<PathBuf>> {
            if formats::detect_path(input).map(|f| f.kind) != Some(Kind::Pdf) {
                return Err(Error::Unsupported("split needs a PDF".to_string()));
            }
            self.check_cancel()?;
            let out_dir = self.options.output_dir.as_deref();
            let base = self.naming_input(input);
            let dir = naming::unique_dir(&naming::dir_for(&base, out_dir), &stem_of(&base))?;
            g.dirs.push(dir.clone());
            let files = guarded(|| pdf::split(input, &dir, self.cancel, progress))?;
            self.check_cancel()?;
            g.outputs.extend(files.iter().cloned());
            Ok(files)
        }

        /// Compress / Trim / Mute / Speed / Crop on one input.
        fn tool_one(
            &self,
            input: &Path,
            op: &Operation,
            progress: &dyn Fn(f32),
            g: &mut Guard,
        ) -> Result<PathBuf> {
            let fmt = formats::detect_path(input).ok_or_else(|| {
                Error::Unsupported(format!("unknown file type: {}", input.display()))
            })?;
            let media = matches!(fmt.kind, Kind::Video | Kind::Audio);
            let unsupported = || Error::Unsupported(format!("{op:?} on {}", fmt.id));
            let (suffix, supported) = match op {
                Operation::Compress { .. } => (
                    " (compressed)",
                    media || matches!(fmt.kind, Kind::Image | Kind::Pdf),
                ),
                Operation::Trim { .. } => (" (trimmed)", media),
                Operation::Mute => (" (muted)", fmt.kind == Kind::Video),
                Operation::Speed { .. } => (" (speed)", media),
                Operation::Crop { .. } => {
                    (" (cropped)", matches!(fmt.kind, Kind::Image | Kind::Video))
                }
                _ => return Err(unsupported()),
            };
            if !supported {
                return Err(unsupported());
            }
            let ff = if media { Some(self.ffmpeg()?) } else { None };
            let ff = || ff.as_ref().ok_or_else(|| missing("ffmpeg"));
            let cancel = self.cancel;
            let out_dir = self.options.output_dir.as_deref();
            let ext = formats::output_ext(fmt.id).unwrap_or(fmt.id);
            let out = naming::output_path(&self.naming_input(input), ext, out_dir, Some(suffix));
            self.produce(g, &out, |tmp| match (op, fmt.kind) {
                (Operation::Compress { level }, Kind::Image) => {
                    image::compress(input, tmp, *level, cancel)
                }
                (Operation::Compress { .. }, Kind::Pdf) => pdf::compress(input, tmp),
                (Operation::Compress { level }, _) => {
                    ff()?.compress(input, tmp, *level, cancel, progress)
                }
                (Operation::Trim { start_ms, end_ms }, _) => {
                    ff()?.trim(input, tmp, *start_ms, *end_ms, cancel, progress)
                }
                (Operation::Mute, _) => ff()?.mute(input, tmp, cancel, progress),
                (Operation::Speed { factor }, _) => {
                    ff()?.speed(input, tmp, *factor, cancel, progress)
                }
                (
                    Operation::Crop {
                        x,
                        y,
                        width,
                        height,
                    },
                    Kind::Image,
                ) => image::crop(input, tmp, *x, *y, *width, *height),
                (
                    Operation::Crop {
                        x,
                        y,
                        width,
                        height,
                    },
                    _,
                ) => ff()?.crop(input, tmp, *x, *y, *width, *height, cancel, progress),
                _ => Err(Error::Unsupported("operation".to_string())),
            })
        }
    }
}
