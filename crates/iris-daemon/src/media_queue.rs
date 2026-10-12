//! Bounded file-only rendering. Database locks must never cross this queue.
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::oneshot;

type Render = Box<dyn FnOnce(&AtomicBool) -> anyhow::Result<Vec<u8>> + Send>;
struct Work {
    priority: u8,
    render: Render,
    cancel: Arc<AtomicBool>,
    result: oneshot::Sender<anyhow::Result<Vec<u8>>>,
}
#[derive(Default)]
struct Queue {
    pending: VecDeque<Work>,
    running: usize,
    background: usize,
}
#[derive(Clone, Default)]
pub(crate) struct MediaQueue(Arc<Mutex<Queue>>);
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl MediaQueue {
    pub(crate) async fn render(
        &self,
        priority: u8,
        render: impl FnOnce(&AtomicBool) -> anyhow::Result<Vec<u8>> + Send + 'static,
    ) -> anyhow::Result<Vec<u8>> {
        let cancel = Arc::new(AtomicBool::new(false));
        let _guard = CancelOnDrop(cancel.clone());
        let (result, receive) = oneshot::channel();
        {
            let mut queue = self.0.lock().unwrap();
            queue.pending.retain(|work| !work.result.is_closed());
            anyhow::ensure!(
                queue.pending.len() < 256,
                "preview queue full; retry the visible photo"
            );
            queue.pending.push_back(Work {
                priority,
                render: Box::new(render),
                cancel,
                result,
            });
        }
        self.dispatch();
        receive.await?
    }

    fn dispatch(&self) {
        let mut queue = self.0.lock().unwrap();
        queue.pending.retain(|work| !work.result.is_closed());
        // Two decoders, at most one speculative preview. Leave room for foreground work.
        while queue.running < 2 {
            let next = queue
                .pending
                .iter()
                .enumerate()
                .filter(|(_, work)| work.priority < 2 || queue.background == 0)
                .min_by_key(|(_, work)| work.priority)
                .map(|(index, _)| index);
            let Some(index) = next else { break };
            let work = queue.pending.remove(index).unwrap();
            queue.running += 1;
            queue.background += usize::from(work.priority == 2);
            let scheduler = self.clone();
            tokio::spawn(async move {
                let background = work.priority == 2;
                let result = tokio::task::spawn_blocking(move || {
                    if work.result.is_closed() || work.cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let _ = work.result.send((work.render)(&work.cancel));
                })
                .await;
                // Even a panicking renderer must release capacity.
                drop(result);
                {
                    let mut queue = scheduler.0.lock().unwrap();
                    queue.running -= 1;
                    queue.background -= usize::from(background);
                }
                scheduler.dispatch();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn foreground_bypasses_speculation_and_abandoned_work_is_not_run() {
        let scheduler = MediaQueue::default();
        let (started, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let background_queue = scheduler.clone();
        let background = tokio::spawn(async move {
            background_queue
                .render(2, move |_| {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                    Ok(vec![1])
                })
                .await
        });
        tokio::task::spawn_blocking(move || ready.recv().unwrap())
            .await
            .unwrap();
        let abandoned_queue = scheduler.clone();
        let executed = Arc::new(AtomicBool::new(false));
        let observed = executed.clone();
        let abandoned = tokio::spawn(async move {
            abandoned_queue
                .render(2, move |_| {
                    observed.store(true, Ordering::Relaxed);
                    Ok(vec![])
                })
                .await
        });
        tokio::task::yield_now().await;
        abandoned.abort();
        let _ = abandoned.await;
        let foreground = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            scheduler.render(0, |_| Ok(vec![2])),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(foreground, vec![2]);
        release.send(()).unwrap();
        assert_eq!(background.await.unwrap().unwrap(), vec![1]);
        scheduler.render(2, |_| Ok(vec![])).await.unwrap();
        assert!(!executed.load(Ordering::Relaxed));
    }
}
