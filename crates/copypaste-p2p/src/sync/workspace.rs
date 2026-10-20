//! Session-local summary storage, separate from the authenticated wire model.

use std::collections::{HashMap, HashSet};

use super::SyncError;
use crate::protocol::ItemSummary;

/// A session-owned snapshot of exactly the summaries sent and received.
/// Implementations may spill to protected storage. All rows, including duplicate
/// remote IDs, are retained in wire order until the session completes.
pub trait SummaryWorkspace: Send {
    /// Whether the next operation may perform blocking storage work.
    fn may_block(&self, _incoming: &[ItemSummary]) -> bool {
        false
    }
    fn add_advertised(&mut self, items: &[ItemSummary]) -> Result<(), SyncError>;
    /// Local metadata captured for IDs in one remote page. This comparison
    /// snapshot must never grant permission to serve an unadvertised row.
    fn add_comparison(&mut self, items: &[ItemSummary]) -> Result<(), SyncError>;
    fn add_remote(&mut self, items: &[ItemSummary]) -> Result<(), SyncError>;
    fn remote_page(
        &mut self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, ItemSummary)>, SyncError>;
    fn advertised(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError>;
    fn comparison(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError>;
    fn wanted(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError>;
    fn set_wanted(&mut self, item: &ItemSummary) -> Result<(), SyncError>;
    fn remote_wins(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        Ok(match self.comparison(&item.item_id)? {
            None => true,
            Some(local) => {
                super::merge_decision_by_summary(&local, item) == super::MergeDecision::TakeRemote
                    || super::pin_state_wins(&local, item)
            }
        })
    }
    fn consider_wanted(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        if self.wanted(&item.item_id)?.is_some_and(|seen| {
            super::merge_decision_by_summary(&seen, item) != super::MergeDecision::TakeRemote
        }) {
            return Ok(false);
        }
        self.set_wanted(item)?;
        Ok(true)
    }
    fn defer(&mut self, id: &str) -> Result<(), SyncError>;
    fn is_deferred(&mut self, id: &str) -> Result<bool, SyncError>;
    /// Keep at most `limit` newest wanted IDs; mark the rest as deferred.
    fn finish_plan(&mut self, limit: usize) -> Result<(Vec<ItemSummary>, usize), SyncError>;
}

/// Default backend for in-memory/test sources. Production sources may override
/// the factory to use bounded, encrypted session storage.
#[derive(Default)]
pub struct MemorySummaryWorkspace {
    advertised: HashMap<String, ItemSummary>,
    comparison: HashMap<String, ItemSummary>,
    remote: Vec<ItemSummary>,
    wanted: HashMap<String, ItemSummary>,
    deferred: HashSet<String>,
}

impl MemorySummaryWorkspace {
    /// Move an existing small session into another backend in bounded batches.
    /// If the target fails, the session must fail rather than omit summaries.
    pub fn spill_into(mut self, target: &mut dyn SummaryWorkspace) -> Result<(), SyncError> {
        let mut batch = Vec::with_capacity(256);
        for (_, item) in self.advertised.drain() {
            batch.push(item);
            if batch.len() == 256 {
                target.add_advertised(&batch)?;
                batch.clear();
            }
        }
        target.add_advertised(&batch)?;
        batch.clear();
        for (_, item) in self.comparison.drain() {
            batch.push(item);
            if batch.len() == 256 {
                target.add_comparison(&batch)?;
                batch.clear();
            }
        }
        target.add_comparison(&batch)?;
        batch.clear();
        for item in self.remote.drain(..) {
            batch.push(item);
            if batch.len() == 256 {
                target.add_remote(&batch)?;
                batch.clear();
            }
        }
        target.add_remote(&batch)?;
        for (_, item) in self.wanted.drain() {
            target.set_wanted(&item)?;
        }
        for id in self.deferred.drain() {
            target.defer(&id)?;
        }
        Ok(())
    }
}

impl SummaryWorkspace for MemorySummaryWorkspace {
    fn remote_wins(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        Ok(match self.comparison.get(&item.item_id) {
            None => true,
            Some(local) => {
                super::merge_decision_by_summary(local, item) == super::MergeDecision::TakeRemote
                    || super::pin_state_wins(local, item)
            }
        })
    }
    fn consider_wanted(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        if self.wanted.get(&item.item_id).is_some_and(|seen| {
            super::merge_decision_by_summary(seen, item) != super::MergeDecision::TakeRemote
        }) {
            return Ok(false);
        }
        self.wanted.insert(item.item_id.clone(), item.clone());
        Ok(true)
    }
    fn add_advertised(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        for item in items {
            self.advertised.insert(item.item_id.clone(), item.clone());
        }
        Ok(())
    }
    fn add_comparison(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        for item in items {
            self.comparison.insert(item.item_id.clone(), item.clone());
        }
        Ok(())
    }
    fn add_remote(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        self.remote.extend_from_slice(items);
        Ok(())
    }
    fn remote_page(
        &mut self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, ItemSummary)>, SyncError> {
        Ok(self
            .remote
            .iter()
            .enumerate()
            .skip(after as usize)
            .take(limit)
            .map(|(i, item)| ((i + 1) as u64, item.clone()))
            .collect())
    }
    fn advertised(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        Ok(self.advertised.get(id).cloned())
    }
    fn comparison(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        Ok(self.comparison.get(id).cloned())
    }
    fn wanted(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        Ok(self.wanted.get(id).cloned())
    }
    fn set_wanted(&mut self, item: &ItemSummary) -> Result<(), SyncError> {
        self.wanted.insert(item.item_id.clone(), item.clone());
        Ok(())
    }
    fn defer(&mut self, id: &str) -> Result<(), SyncError> {
        self.deferred.insert(id.to_owned());
        Ok(())
    }
    fn is_deferred(&mut self, id: &str) -> Result<bool, SyncError> {
        Ok(self.deferred.contains(id))
    }
    fn finish_plan(&mut self, limit: usize) -> Result<(Vec<ItemSummary>, usize), SyncError> {
        let mut ordered: Vec<_> = self.wanted.values().cloned().collect();
        ordered.sort_unstable_by_key(|s| std::cmp::Reverse(s.created_at));
        let dropped = ordered.len().saturating_sub(limit);
        for item in ordered.iter().skip(limit) {
            self.deferred.insert(item.item_id.clone());
        }
        ordered.truncate(limit);
        Ok((ordered, dropped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct DroppableWorkspace(Arc<AtomicBool>, MemorySummaryWorkspace);
    impl Drop for DroppableWorkspace {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    impl SummaryWorkspace for DroppableWorkspace {
        fn may_block(&self, _: &[ItemSummary]) -> bool {
            true
        }
        fn add_advertised(&mut self, x: &[ItemSummary]) -> Result<(), SyncError> {
            self.1.add_advertised(x)
        }
        fn add_comparison(&mut self, x: &[ItemSummary]) -> Result<(), SyncError> {
            self.1.add_comparison(x)
        }
        fn add_remote(&mut self, x: &[ItemSummary]) -> Result<(), SyncError> {
            self.1.add_remote(x)
        }
        fn remote_page(&mut self, a: u64, l: usize) -> Result<Vec<(u64, ItemSummary)>, SyncError> {
            self.1.remote_page(a, l)
        }
        fn advertised(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
            self.1.advertised(id)
        }
        fn comparison(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
            self.1.comparison(id)
        }
        fn wanted(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
            self.1.wanted(id)
        }
        fn set_wanted(&mut self, s: &ItemSummary) -> Result<(), SyncError> {
            self.1.set_wanted(s)
        }
        fn defer(&mut self, id: &str) -> Result<(), SyncError> {
            self.1.defer(id)
        }
        fn is_deferred(&mut self, id: &str) -> Result<bool, SyncError> {
            self.1.is_deferred(id)
        }
        fn finish_plan(&mut self, l: usize) -> Result<(Vec<ItemSummary>, usize), SyncError> {
            self.1.finish_plan(l)
        }
    }

    async fn cancelled_io_keeps_ownership() {
        let dropped = Arc::new(AtomicBool::new(false));
        let workspace: Box<dyn SummaryWorkspace> = Box::new(DroppableWorkspace(
            Arc::clone(&dropped),
            MemorySummaryWorkspace::default(),
        ));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(async move {
            super::super::workspace_call(workspace, true, move |_| {
                let _ = started_tx.send(());
                release_rx.recv().unwrap();
            })
            .await
        });
        started_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        assert!(!dropped.load(Ordering::Acquire));
        release_tx.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_io_current_thread_keeps_workspace_until_worker_exits() {
        cancelled_io_keeps_ownership().await;
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_io_multi_thread_keeps_workspace_until_worker_exits() {
        cancelled_io_keeps_ownership().await;
    }
}
