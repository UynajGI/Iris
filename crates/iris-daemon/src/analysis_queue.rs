use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};
use utoipa::ToSchema;

/// Browsing changes only queued work, never starts a job or changes its scope.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BrowseContext {
    pub focused_photo_id: Option<i64>,
    pub comparison_photo_ids: Vec<i64>,
    pub nearby_photo_ids: Vec<i64>,
    /// Stable id for one window; monotonically increasing revisions reject late requests.
    pub client_id: String,
    pub revision: u64,
}

impl BrowseContext {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.comparison_photo_ids.len() <= 32,
            "too many comparison photos"
        );
        anyhow::ensure!(self.nearby_photo_ids.len() <= 128, "too many nearby photos");
        anyhow::ensure!(self.client_id.len() <= 64, "client id is too long");
        anyhow::ensure!(
            self.ordered_ids().iter().all(|id| *id > 0),
            "photo ids must be positive"
        );
        Ok(())
    }

    pub(crate) fn ordered_ids(&self) -> Vec<i64> {
        let mut seen = HashSet::new();
        self.focused_photo_id
            .into_iter()
            .chain(self.comparison_photo_ids.iter().copied())
            .chain(self.nearby_photo_ids.iter().copied())
            .filter(|id| seen.insert(*id))
            .collect()
    }
}

#[derive(Default)]
pub(crate) struct BrowseContexts(Mutex<HashMap<i64, (BrowseContext, Instant)>>);

impl BrowseContexts {
    pub(crate) fn update(&self, project: i64, context: BrowseContext) -> BrowseContext {
        let mut contexts = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some((prior, _)) = contexts.get(&project) {
            if prior.client_id == context.client_id
                && context.revision > 0
                && context.revision <= prior.revision
            {
                return prior.clone();
            }
        }
        contexts.insert(project, (context.clone(), Instant::now()));
        context
    }

    pub(crate) fn current(&self, project: i64) -> BrowseContext {
        self.current_at(project, Instant::now())
    }

    fn current_at(&self, project: i64, now: Instant) -> BrowseContext {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&project)
            .filter(|(_, received)| {
                now.saturating_duration_since(*received) < Duration::from_secs(30)
            })
            .map(|(context, _)| context.clone())
            .unwrap_or_default()
    }
}

/// Single claim boundary shared by all workers. Priority scans at most 161 ids;
/// FIFO entries are removed once, so background dispatch is amortized O(1).
pub(crate) struct PendingOrder {
    remaining: HashMap<i64, usize>,
    fifo: VecDeque<i64>,
    foreground_claims: usize,
}

impl PendingOrder {
    pub(crate) fn new(ids: impl IntoIterator<Item = i64>) -> Self {
        let mut remaining = HashMap::new();
        let mut fifo = VecDeque::new();
        for (index, id) in ids.into_iter().enumerate() {
            if let std::collections::hash_map::Entry::Vacant(entry) = remaining.entry(id) {
                entry.insert(index);
                fifo.push_back(id);
            }
        }
        Self {
            remaining,
            fifo,
            foreground_claims: 0,
        }
    }

    pub(crate) fn claim(&mut self, context: &BrowseContext) -> Option<usize> {
        // One FIFO turn after four foreground claims bounds starvation even
        // when users continually change focus during a long-running analysis.
        if self.foreground_claims < 4 {
            for id in context.ordered_ids() {
                if let Some(index) = self.remaining.remove(&id) {
                    self.foreground_claims += 1;
                    return Some(index);
                }
            }
        }
        self.foreground_claims = 0;
        while let Some(id) = self.fifo.pop_front() {
            if let Some(index) = self.remaining.remove(&id) {
                return Some(index);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_comparison_neighbors_and_fifo_have_distinct_priorities() {
        let mut queue = PendingOrder::new(1..=8);
        let context = BrowseContext {
            focused_photo_id: Some(6),
            comparison_photo_ids: vec![7, 6],
            nearby_photo_ids: vec![8, 7],
            ..Default::default()
        };
        let actual: Vec<_> = std::iter::from_fn(|| queue.claim(&context)).collect();
        assert_eq!(actual, vec![5, 6, 7, 0, 1, 2, 3, 4]);
    }

    #[test]
    fn changed_focus_reorders_only_unclaimed_work_and_keeps_background_fair() {
        let mut queue = PendingOrder::new(1..=20);
        for id in 10..=15 {
            let context = BrowseContext {
                focused_photo_id: Some(id),
                ..Default::default()
            };
            let index = queue.claim(&context).unwrap();
            if id == 14 {
                assert_eq!(index, 0);
            } else {
                assert_eq!(index, (id - 1) as usize);
            }
        }
        let rest: Vec<_> = std::iter::from_fn(|| queue.claim(&BrowseContext::default())).collect();
        assert_eq!(rest.len(), 14);
        assert!(rest.contains(&13)); // Focus skipped for fairness is still pending.
    }

    #[test]
    fn concurrent_claims_execute_each_pending_photo_once() {
        let queue = std::sync::Arc::new(Mutex::new(PendingOrder::new(1..=1000)));
        let context = BrowseContext {
            focused_photo_id: Some(900),
            nearby_photo_ids: (901..=950).collect(),
            ..Default::default()
        };
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let queue = queue.clone();
                    let context = &context;
                    scope.spawn(move || {
                        std::iter::from_fn(|| queue.lock().unwrap().claim(context))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(results.len(), 1000);
        assert_eq!(
            results.into_iter().collect::<HashSet<_>>(),
            (0..1000).collect()
        );
    }

    #[test]
    fn stale_requests_cannot_restore_old_focus_and_context_expires() {
        let contexts = BrowseContexts::default();
        let mut newer = BrowseContext {
            focused_photo_id: Some(2),
            client_id: "window".into(),
            revision: 2,
            ..Default::default()
        };
        contexts.update(1, newer.clone());
        let older = BrowseContext {
            focused_photo_id: Some(1),
            revision: 1,
            ..newer.clone()
        };
        assert_eq!(contexts.update(1, older), newer);
        assert_eq!(contexts.current(2), BrowseContext::default());
        assert_eq!(
            contexts.current_at(1, Instant::now() + Duration::from_secs(31)),
            BrowseContext::default()
        );
        newer.focused_photo_id = None;
        newer.revision = 3;
        contexts.update(1, newer.clone());
        assert_eq!(contexts.current(1), newer);
    }
}
