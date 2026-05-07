use parking_lot::Mutex;
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Clone)]
pub struct MemoryTracker {
    counters: Arc<Mutex<HashMap<String, usize>>>,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn track_allocation(&self, component: &str, bytes: usize) {
        let mut counters = self.counters.lock();
        *counters.entry(component.to_string()).or_insert(0) += bytes;
    }

    pub fn track_deallocation(&self, component: &str, bytes: usize) {
        let mut counters = self.counters.lock();
        if let Some(count) = counters.get_mut(component) {
            *count = count.saturating_sub(bytes);
        }
    }

    pub fn get_snapshot(&self) -> Vec<(String, usize)> {
        let counters = self.counters.lock();
        let mut snapshot: Vec<_> = counters.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        snapshot.sort_by(|a, b| b.1.cmp(&a.1));
        snapshot
    }

    pub fn estimate_size<T>(&self, item: &T, component: &str) -> usize {
        let size = std::mem::size_of_val(item);
        self.track_allocation(component, size);
        size
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}
