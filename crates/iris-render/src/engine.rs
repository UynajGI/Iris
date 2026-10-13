// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared linear source/proxy cache and latest-wins preview sessions. Pixel work
//! runs outside state locks. Cache limits cover sources AND FP32 proxies; eviction
//! drops cache ownership, while the shared budget tracks every surviving Arc.
//! Retired sequence watermarks retain the last 256 session identifiers; clients
//! must use fresh identifiers after closing a session or leaving that bounded history.
use crate::{
    document::{hash_bytes, EditDocument},
    graph::Resources,
    Frame, RenderContext,
};
use anyhow::{anyhow, bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenderQuality {
    Interactive,
    Full,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderRequest {
    pub revision: u64,
    pub session: String,
    pub sequence: u64,
    pub quality: RenderQuality,
    pub max_edge: u32,
}
#[derive(Debug)]
pub struct RenderedFrame {
    pub frame: Arc<Frame>,
    pub revision: u64,
    pub sequence: u64,
    pub fingerprint: String,
    pub approximate: bool,
    pub elapsed_ms: u64,
    pub execution: crate::graph::ExecutionTrace,
    // Retain the budget itself while a result wrapper lives. Admission belongs to
    // the frame's Weak tracking entry, so cloning frame does not lose its charge.
    _budget: RenderContext,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CacheKey {
    Source(String),
    Proxy(String, u32),
}
struct CachedFrame {
    frame: Arc<Frame>,
    touched: u64,
}
struct Session {
    sequence: u64,
    context: RenderContext,
    started: bool,
    active: bool,
    external: bool,
    touched: u64,
}
struct Watermark {
    sequence: u64,
    touched: u64,
}
struct State {
    frames: BTreeMap<CacheKey, CachedFrame>,
    flights: BTreeMap<CacheKey, Arc<Flight>>,
    sessions: BTreeMap<String, Session>,
    retired: BTreeMap<String, Watermark>,
    clock: u64,
    cache_epoch: u64,
}
struct Flight {
    result: Mutex<Option<FlightResult>>,
    ready: Condvar,
}
enum FlightResult {
    Ready(Arc<Frame>),
    Failed { message: String, retry: bool },
}
impl Flight {
    fn wait(&self, context: &RenderContext) -> Result<Option<Arc<Frame>>> {
        let mut result = self
            .result
            .lock()
            .map_err(|_| anyhow!("source flight poisoned"))?;
        loop {
            context.check_cancelled()?;
            match result.as_ref() {
                Some(FlightResult::Ready(frame)) => return Ok(Some(frame.clone())),
                Some(FlightResult::Failed { retry: true, .. }) => return Ok(None),
                Some(FlightResult::Failed {
                    message,
                    retry: false,
                }) => bail!("shared source/proxy load failed: {message}"),
                None => {
                    result = self
                        .ready
                        .wait_timeout(result, Duration::from_millis(20))
                        .map_err(|_| anyhow!("source flight poisoned"))?
                        .0
                }
            }
        }
    }
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            ready: Condvar::new(),
        }
    }
}
const MAX_SESSIONS: usize = 64;
const MAX_WATERMARKS: usize = 256;
const MAX_CACHE_ENTRIES: usize = 512;
const MAX_FLIGHTS: usize = 128;

pub struct RenderEngine {
    context: RenderContext,
    cache_limit: u64,
    state: Mutex<State>,
    executor: crate::graph::Executor,
}
impl RenderEngine {
    /// Explicit CPU baseline; the default constructor still permits GPU execution.
    pub fn new_cpu(memory_limit: u64, source_cache_limit: u64) -> Result<Self> {
        let mut engine = Self::new(memory_limit, source_cache_limit)?;
        engine.executor = crate::graph::Executor::cpu_only();
        Ok(engine)
    }
    pub fn new(memory_limit: u64, source_cache_limit: u64) -> Result<Self> {
        ensure!(
            (64 * 1024 * 1024..=64 * 1024 * 1024 * 1024).contains(&memory_limit),
            "invalid renderer memory limit"
        );
        ensure!(
            source_cache_limit <= memory_limit / 2,
            "source/proxy cache must leave working memory"
        );
        Ok(Self {
            context: RenderContext::new(memory_limit),
            cache_limit: source_cache_limit,
            state: Mutex::new(State {
                frames: BTreeMap::new(),
                flights: BTreeMap::new(),
                sessions: BTreeMap::new(),
                retired: BTreeMap::new(),
                clock: 0,
                cache_epoch: 0,
            }),
            executor: crate::graph::Executor::default(),
        })
    }
    pub fn memory_used(&self) -> u64 {
        self.context.memory_used()
    }
    pub fn request_context(&self) -> RenderContext {
        self.context.child()
    }
    pub fn check_request(&self, session: &str, sequence: u64) -> Result<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        let active = state
            .sessions
            .get(session)
            .ok_or_else(|| anyhow!("render session closed"))?;
        ensure!(active.sequence == sequence, "stale render result");
        active.context.check_cancelled()
    }
    pub fn cancel_request(&self, session: &str, sequence: u64) -> Result<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        if let Some(active) = state
            .sessions
            .get(session)
            .filter(|s| s.sequence == sequence)
        {
            active.context.cancel();
        }
        Ok(())
    }
    /// Hosts which pre-register before queuing should call this after response
    /// encoding/scopes finish, including full-resolution scopes outside the engine.
    /// A late completion for an old sequence never changes the newer session.
    pub fn finish_request(&self, session: &str, sequence: u64) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        state.clock = state.clock.saturating_add(1);
        let touched = state.clock;
        if let Some(active) = state
            .sessions
            .get_mut(session)
            .filter(|s| s.sequence == sequence)
        {
            active.active = false;
            active.started = true;
            active.touched = touched;
        }
        Ok(())
    }
    /// Explicit close releases the sequence watermark. Use a new session identifier
    /// on reopen so old network messages cannot collide with a reused sequence.
    pub fn cancel_session(&self, session: &str) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        if let Some(active) = state.sessions.remove(session) {
            active.context.cancel();
        }
        state.retired.remove(session);
        Ok(())
    }
    pub fn clear_cache(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        state.frames.clear();
        state.cache_epoch = state.cache_epoch.wrapping_add(1);
        drop(state);
        self.context.memory_used();
        Ok(())
    }
    pub fn register_request(
        &self,
        request: &RenderRequest,
        parent: &RenderContext,
    ) -> Result<RenderContext> {
        self.install_request(request, parent, false)
    }
    fn begin(&self, request: &RenderRequest, parent: &RenderContext) -> Result<RenderContext> {
        self.install_request(request, parent, true)
    }
    fn install_request(
        &self,
        request: &RenderRequest,
        parent: &RenderContext,
        start: bool,
    ) -> Result<RenderContext> {
        parent.check_cancelled()?;
        ensure!(
            !request.session.is_empty() && request.session.len() <= 128,
            "invalid render session"
        );
        ensure!(
            (16..=8192).contains(&request.max_edge),
            "preview edge out of bounds"
        );
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("renderer state poisoned"))?;
        state.clock = state.clock.saturating_add(1);
        let touched = state.clock;
        if let Some(previous) = state.sessions.get_mut(&request.session) {
            if start && previous.sequence == request.sequence && !previous.started {
                previous.context.check_cancelled()?;
                previous.started = true;
                previous.active = true;
                previous.touched = touched;
                return Ok(previous.context.linked_to(parent));
            }
            ensure!(request.sequence > previous.sequence, "stale render request");
            previous.context.cancel();
        } else {
            if let Some(previous) = state.retired.get(&request.session) {
                ensure!(
                    request.sequence > previous.sequence,
                    "stale retired render request"
                );
            }
            // Never evict queued/running/encoding work. Completed LRU sessions and
            // already cancelled sessions can be retired without cancelling others.
            while state.sessions.len() >= MAX_SESSIONS {
                let oldest = state
                    .sessions
                    .iter()
                    .filter(|(_, s)| !s.active || s.context.check_cancelled().is_err())
                    .min_by_key(|(_, s)| s.touched)
                    .map(|(key, _)| key.clone());
                let Some(oldest) = oldest else {
                    bail!("too many active render sessions");
                };
                let previous = state.sessions.remove(&oldest).unwrap();
                state.retired.insert(
                    oldest,
                    Watermark {
                        sequence: previous.sequence,
                        touched,
                    },
                );
                while state.retired.len() > MAX_WATERMARKS {
                    let key = state
                        .retired
                        .iter()
                        .min_by_key(|(_, s)| s.touched)
                        .map(|(key, _)| key.clone())
                        .unwrap();
                    state.retired.remove(&key);
                }
            }
        }
        state.retired.remove(&request.session);
        let context = self.context.child().linked_to(parent);
        state.sessions.insert(
            request.session.clone(),
            Session {
                sequence: request.sequence,
                context: context.clone(),
                started: start,
                active: true,
                external: !start,
                touched,
            },
        );
        Ok(context)
    }

    fn frame(
        &self,
        key: CacheKey,
        context: &RenderContext,
        load: impl FnOnce() -> Result<Arc<Frame>>,
    ) -> Result<Arc<Frame>> {
        let mut load = Some(load);
        loop {
            context.check_cancelled()?;
            let (flight, leader, epoch) = {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| anyhow!("renderer state poisoned"))?;
                state.clock = state.clock.saturating_add(1);
                let touched = state.clock;
                if let Some(cached) = state.frames.get_mut(&key) {
                    cached.touched = touched;
                    return Ok(cached.frame.clone());
                }
                if let Some(flight) = state.flights.get(&key) {
                    (flight.clone(), false, state.cache_epoch)
                } else {
                    ensure!(
                        state.flights.len() < MAX_FLIGHTS,
                        "too many in-flight source/proxy loads"
                    );
                    let flight = Arc::new(Flight::new());
                    state.flights.insert(key.clone(), flight.clone());
                    (flight, true, state.cache_epoch)
                }
            };
            if leader {
                let guard = FlightLeader {
                    engine: self,
                    key: key.clone(),
                    flight: flight.clone(),
                    epoch,
                    finished: false,
                };
                let result = (|| {
                    let frame = load
                        .take()
                        .ok_or_else(|| anyhow!("source loader already consumed"))?(
                    )?;
                    context.check_cancelled()?;
                    context.track_frame(&frame)?;
                    Ok(frame)
                })();
                let retry = context.check_cancelled().is_err();
                guard.finish(&result, retry)?;
                return result;
            }
            if let Some(frame) = flight.wait(context)? {
                return Ok(frame);
            }
            // The previous producer was superseded/cancelled. A live waiter may
            // retry with its own still-unused loader; it never inherits cancellation.
        }
    }

    pub fn render(
        &self,
        document: &EditDocument,
        request: &RenderRequest,
        load: impl FnOnce(&RenderContext) -> Result<Frame>,
        resources: &impl Resources,
    ) -> Result<RenderedFrame> {
        self.render_with_context(document, request, load, resources, &self.request_context())
    }
    fn render_source(
        &self,
        document: &EditDocument,
        request: &RenderRequest,
        load: impl FnOnce(&RenderContext) -> Result<Frame>,
        context: &RenderContext,
    ) -> Result<(Arc<Frame>, bool)> {
        let source_key = hash_bytes(&serde_json::to_vec(&document.source)?);
        let approximate = request.quality == RenderQuality::Interactive
            && document.source.width.max(document.source.height) > request.max_edge;
        let source_loader = || {
            self.frame(CacheKey::Source(source_key.clone()), context, || {
                let frame = Arc::new(load(context)?);
                frame.validate()?;
                ensure!(
                    frame.width == document.source.width && frame.height == document.source.height,
                    "decoded dimensions changed"
                );
                Ok(frame)
            })
        };
        // Look up the proxy first: slider edits need not touch or even retain the
        // original source if its exact SourceBinding/max-edge proxy is cached.
        let source = if approximate {
            self.frame(
                CacheKey::Proxy(source_key.clone(), request.max_edge),
                context,
                || {
                    let full = source_loader()?;
                    Ok(Arc::new(full.proxy(request.max_edge, context)?))
                },
            )?
        } else {
            source_loader()?
        };
        Ok((source, approximate))
    }
    pub fn render_with_context(
        &self,
        document: &EditDocument,
        request: &RenderRequest,
        load: impl FnOnce(&RenderContext) -> Result<Frame>,
        resources: &impl Resources,
        parent: &RenderContext,
    ) -> Result<RenderedFrame> {
        document.validate()?;
        let fingerprint = document.fingerprint()?;
        let context = self.begin(request, parent)?;
        let mut finish = RequestFinish {
            engine: self,
            session: &request.session,
            sequence: request.sequence,
            context: context.clone(),
            success: false,
        };
        let start = Instant::now();
        let (source, approximate) = self.render_source(document, request, load, &context)?;
        context.check_cancelled()?;
        let mut render_document = document.clone();
        let context = if approximate {
            let scale = (source.width as f32 / document.source.width as f32)
                .min(source.height as f32 / document.source.height as f32);
            render_document.source.width = source.width;
            render_document.source.height = source.height;
            context.with_pixel_scale(scale)?
        } else {
            context
        };
        let (rendered, execution) = crate::graph::render_with_executor(
            &render_document,
            source,
            resources,
            &context,
            &self.executor,
        )?;
        let output = if rendered.width.max(rendered.height) > request.max_edge {
            Arc::new(rendered.proxy(request.max_edge, &context)?)
        } else {
            rendered
        };
        context.check_cancelled()?;
        context.track_frame(&output)?;
        finish.success = true;
        Ok(RenderedFrame {
            frame: output,
            revision: request.revision,
            sequence: request.sequence,
            fingerprint,
            approximate,
            execution,
            elapsed_ms: start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            _budget: context,
        })
    }
}

struct FlightLeader<'a> {
    engine: &'a RenderEngine,
    key: CacheKey,
    flight: Arc<Flight>,
    epoch: u64,
    finished: bool,
}
impl FlightLeader<'_> {
    fn finish(mut self, result: &Result<Arc<Frame>>, retry: bool) -> Result<()> {
        {
            let mut state = self
                .engine
                .state
                .lock()
                .map_err(|_| anyhow!("renderer state poisoned"))?;
            let current = state
                .flights
                .get(&self.key)
                .is_some_and(|flight| Arc::ptr_eq(flight, &self.flight));
            if current {
                state.flights.remove(&self.key);
                if let Ok(frame) = result {
                    if state.cache_epoch == self.epoch
                        && frame.byte_len() <= self.engine.cache_limit
                    {
                        while state.frames.len() >= MAX_CACHE_ENTRIES
                            || state
                                .frames
                                .values()
                                .map(|f| f.frame.byte_len())
                                .sum::<u64>()
                                + frame.byte_len()
                                > self.engine.cache_limit
                        {
                            let oldest = state
                                .frames
                                .iter()
                                .min_by_key(|(_, f)| f.touched)
                                .map(|(key, _)| key.clone());
                            if let Some(oldest) = oldest {
                                state.frames.remove(&oldest);
                            } else {
                                break;
                            }
                        }
                        state.clock = state.clock.saturating_add(1);
                        let touched = state.clock;
                        // A single flight publishes a key once; never replace a
                        // completed entry with a duplicate decoder's reservation.
                        state
                            .frames
                            .entry(self.key.clone())
                            .or_insert_with(|| CachedFrame {
                                frame: frame.clone(),
                                touched,
                            });
                    }
                }
            }
            // Publish while holding state: a new flight cannot start between
            // removal and making the old producer's result visible to waiters.
            *self
                .flight
                .result
                .lock()
                .map_err(|_| anyhow!("source flight poisoned"))? = Some(match result {
                Ok(frame) => FlightResult::Ready(frame.clone()),
                Err(error) => FlightResult::Failed {
                    message: error.to_string(),
                    retry,
                },
            });
        }
        self.finished = true;
        self.flight.ready.notify_all();
        Ok(())
    }
}
impl Drop for FlightLeader<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Ok(mut state) = self.engine.state.lock() {
            if state
                .flights
                .get(&self.key)
                .is_some_and(|flight| Arc::ptr_eq(flight, &self.flight))
            {
                state.flights.remove(&self.key);
            }
        }
        if let Ok(mut result) = self.flight.result.lock() {
            *result = Some(FlightResult::Failed {
                message: "source producer abandoned".into(),
                retry: true,
            });
        }
        self.flight.ready.notify_all();
    }
}
struct RequestFinish<'a> {
    engine: &'a RenderEngine,
    session: &'a str,
    sequence: u64,
    context: RenderContext,
    success: bool,
}
impl Drop for RequestFinish<'_> {
    fn drop(&mut self) {
        if !self.success {
            self.context.cancel();
        }
        if let Ok(mut state) = self.engine.state.lock() {
            state.clock = state.clock.saturating_add(1);
            let touched = state.clock;
            if let Some(session) = state
                .sessions
                .get_mut(self.session)
                .filter(|s| s.sequence == self.sequence && s.context.same_request(&self.context))
            {
                // Pre-registered network work remains active through encoding;
                // only the host's sequence-checked finish_request ends its lease.
                if !session.external || !self.success {
                    session.active = false;
                }
                session.touched = touched;
            }
        }
    }
}
// Private flight tests coordinate on ownership rather than timing-dependent sleeps.
#[cfg(test)]
mod flight_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };

    fn wait_for_follower(engine: &RenderEngine, key: &CacheKey) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let joined = engine
                .state
                .lock()
                .unwrap()
                .flights
                .get(key)
                .is_some_and(|flight| Arc::strong_count(flight) >= 4);
            if joined {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "follower did not join shared flight"
            );
            std::thread::yield_now();
        }
    }
    fn pixel() -> Arc<Frame> {
        Arc::new(Frame::new(1, 1, vec![[-0.2, 0.12345, 2.0, 1.0]]).unwrap())
    }
    #[test]
    fn overlapping_uncached_loads_share_one_allocation() {
        let engine = RenderEngine::new(64 * 1024 * 1024, 0).unwrap();
        let calls = AtomicUsize::new(0);
        let key = CacheKey::Source("shared".into());
        let (entered, started) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        std::thread::scope(|scope| {
            let first = scope.spawn({
                let engine = &engine;
                let key = &key;
                let calls = &calls;
                move || {
                    engine.frame(key.clone(), &engine.request_context(), || {
                        calls.fetch_add(1, Ordering::SeqCst);
                        entered.send(()).unwrap();
                        blocked.recv_timeout(Duration::from_secs(10)).unwrap();
                        Ok(pixel())
                    })
                }
            });
            started.recv_timeout(Duration::from_secs(5)).unwrap();
            let second = scope.spawn(|| {
                engine.frame(key.clone(), &engine.request_context(), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(pixel())
                })
            });
            wait_for_follower(&engine, &key);
            release.send(()).unwrap();
            let a = first.join().unwrap().unwrap();
            let b = second.join().unwrap().unwrap();
            assert!(Arc::ptr_eq(&a, &b));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(engine.memory_used(), 16);
        });
        assert_eq!(engine.memory_used(), 0);
    }
    #[test]
    fn cancelled_producer_does_not_cancel_live_waiter() {
        let engine = RenderEngine::new(64 * 1024 * 1024, 0).unwrap();
        let key = CacheKey::Source("retry".into());
        let cancelled = engine.request_context();
        let (entered, started) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        std::thread::scope(|scope| {
            let first = scope.spawn({
                let engine = &engine;
                let key = &key;
                let cancelled = &cancelled;
                move || {
                    engine.frame(key.clone(), cancelled, || {
                        entered.send(()).unwrap();
                        blocked.recv_timeout(Duration::from_secs(10)).unwrap();
                        cancelled.check_cancelled()?;
                        Ok(pixel())
                    })
                }
            });
            started.recv_timeout(Duration::from_secs(5)).unwrap();
            let second = scope
                .spawn(|| engine.frame(key.clone(), &engine.request_context(), || Ok(pixel())));
            wait_for_follower(&engine, &key);
            cancelled.cancel();
            release.send(()).unwrap();
            assert!(first.join().unwrap().is_err());
            assert_eq!(second.join().unwrap().unwrap().pixels[0][2], 2.0);
        });
        assert_eq!(engine.memory_used(), 0);
    }
}
