use std::collections::BinaryHeap;
use std::cmp::Ordering;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::agentd::graph::{TaskNode, TaskStatus};

/// Priority wrapper for the task queue.
struct PrioritizedTask {
    priority: u8,
    id: Uuid,
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool { self.priority == other.priority }
}
impl Eq for PrioritizedTask {}
impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering { self.priority.cmp(&other.priority) }
}

pub struct TaskQueue {
    heap: Mutex<BinaryHeap<PrioritizedTask>>,
    tasks: Mutex<std::collections::HashMap<Uuid, TaskNode>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            heap: Mutex::new(BinaryHeap::new()),
            tasks: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub async fn push(&self, task: TaskNode) {
        let priority = task.priority;
        let id = task.id;
        self.tasks.lock().await.insert(id, task);
        self.heap.lock().await.push(PrioritizedTask { priority, id });
    }

    /// Pop the highest-priority pending task whose parents are all complete.
    pub async fn pop_ready(&self) -> Option<TaskNode> {
        let tasks = self.tasks.lock().await;
        let mut heap = self.heap.lock().await;

        // Drain and re-collect: find first ready task.
        let mut candidates: Vec<PrioritizedTask> = heap.drain().collect();
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut ready_idx = None;
        for (i, c) in candidates.iter().enumerate() {
            if let Some(task) = tasks.get(&c.id) {
                if task.status != TaskStatus::Pending { continue; }
                let parents_done = task.parent_ids.iter().all(|pid| {
                    tasks.get(pid).map(|p| p.status == TaskStatus::Complete).unwrap_or(true)
                });
                if parents_done {
                    ready_idx = Some(i);
                    break;
                }
            }
        }

        match ready_idx {
            None => {
                for c in candidates { heap.push(c); }
                None
            }
            Some(i) => {
                let chosen = candidates.remove(i);
                for c in candidates { heap.push(c); }
                drop(heap);
                drop(tasks);
                self.tasks.lock().await.remove(&chosen.id)
            }
        }
    }

    pub async fn len(&self) -> usize {
        self.heap.lock().await.len()
    }
}
