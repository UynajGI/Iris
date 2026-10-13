// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{bail, ensure, Result};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, Weak,
};

#[derive(Clone, Debug)]
pub struct RenderContext {
    cancelled: Arc<AtomicBool>,
    budget: Arc<Budget>,
    pixel_scale: f32,
    linked_cancellation: Vec<Arc<AtomicBool>>,
}

#[derive(Debug)]
struct Budget {
    limit: u64,
    used: AtomicU64,
    frames: Mutex<BTreeMap<usize, TrackedFrame>>,
}

#[derive(Debug)]
struct TrackedFrame {
    frame: Weak<crate::Frame>,
    bytes: u64,
}

impl Budget {
    fn reclaim(&self, frames: &mut BTreeMap<usize, TrackedFrame>) {
        frames.retain(|_, entry| {
            if entry.frame.strong_count() == 0 {
                self.used.fetch_sub(entry.bytes, Ordering::AcqRel);
                false
            } else {
                true
            }
        });
    }
    fn admit(&self, bytes: u64) -> Result<()> {
        let result = self
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|next| *next <= self.limit)
            });
        if result.is_err() {
            bail!("render memory budget exceeded: requested {bytes} bytes");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct MemoryReservation {
    budget: Arc<Budget>,
    bytes: u64,
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::new(1024 * 1024 * 1024)
    }
}

impl RenderContext {
    pub fn new(memory_limit_bytes: u64) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            budget: Arc::new(Budget {
                limit: memory_limit_bytes,
                used: AtomicU64::new(0),
                frames: Mutex::new(BTreeMap::new()),
            }),
            pixel_scale: 1.0,
            linked_cancellation: Vec::new(),
        }
    }
    /// Share an admission budget with a new independently cancellable request.
    pub fn child(&self) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            budget: self.budget.clone(),
            pixel_scale: self.pixel_scale,
            linked_cancellation: Vec::new(),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn cancellation_flag(&self) -> &AtomicBool {
        &self.cancelled
    }
    pub fn check_cancelled(&self) -> Result<()> {
        ensure!(
            !self.cancelled.load(Ordering::Acquire)
                && !self
                    .linked_cancellation
                    .iter()
                    .any(|flag| flag.load(Ordering::Acquire)),
            "render cancelled"
        );
        Ok(())
    }
    pub fn linked_to(&self, parent: &Self) -> Self {
        let mut child = self.clone();
        child.linked_cancellation.push(parent.cancelled.clone());
        child
            .linked_cancellation
            .extend(parent.linked_cancellation.iter().cloned());
        child
    }
    pub fn memory_limit(&self) -> u64 {
        self.budget.limit
    }
    pub fn pixel_scale(&self) -> f32 {
        self.pixel_scale
    }
    pub fn with_pixel_scale(&self, scale: f32) -> Result<Self> {
        ensure!(
            scale.is_finite() && scale > 0.0 && scale <= 1.0,
            "invalid render scale"
        );
        let mut context = self.clone();
        context.pixel_scale = scale;
        Ok(context)
    }
    pub fn memory_used(&self) -> u64 {
        // Weak tracking never releases a live Arc's admission. Reclamation is
        // lazy until the next budget interaction, conservatively overcounting
        // the interval after the final strong reference is dropped.
        if let Ok(mut frames) = self.budget.frames.lock() {
            self.budget.reclaim(&mut frames);
        }
        self.budget.used.load(Ordering::Acquire)
    }
    /// Keep one admission per shared frame allocation, independent of cache entry
    /// or result wrapper lifetime. Host clones of Arc<Frame> remain accounted.
    /// Producer scratch guards end before this handoff; allocator/driver overhead
    /// and the short concurrent return-to-track gap are not a hard RSS guarantee.
    pub fn track_frame(&self, frame: &Arc<crate::Frame>) -> Result<()> {
        self.check_cancelled()?;
        let mut frames = self
            .budget
            .frames
            .lock()
            .map_err(|_| anyhow::anyhow!("frame budget poisoned"))?;
        self.budget.reclaim(&mut frames);
        let key = Arc::as_ptr(frame) as usize;
        if frames.contains_key(&key) {
            return Ok(());
        }
        let bytes = frame.byte_len();
        self.budget.admit(bytes)?;
        frames.insert(
            key,
            TrackedFrame {
                frame: Arc::downgrade(frame),
                bytes,
            },
        );
        Ok(())
    }
    pub(crate) fn same_request(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
    pub fn reserve(&self, bytes: u64) -> Result<MemoryReservation> {
        self.check_cancelled()?;
        {
            let mut frames = self
                .budget
                .frames
                .lock()
                .map_err(|_| anyhow::anyhow!("frame budget poisoned"))?;
            self.budget.reclaim(&mut frames);
        }
        self.budget.admit(bytes)?;
        Ok(MemoryReservation {
            budget: self.budget.clone(),
            bytes,
        })
    }
}
