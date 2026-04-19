//! plato-tile-priority — Priority queue for PLATO tiles with deadband P0/P1/P2 ordering
//!
//! Implements Oracle1's Deadband Protocol as a priority queue:
//! P0 (rocks/negatives) always first, P1 (channels) next, P2 (optimize) last.
//! Within each level, FIFO ordering. Urgency override for critical tiles.

/// Deadband priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    P0,  // Rocks — must address NOW (negative space, safety)
    P1,  // Channels — safe paths to explore (routing, communication)
    P2,  // Optimize — improvements when bandwidth allows
}

impl Default for Priority { fn default() -> Self { Priority::P2 } }

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Priority::P0 => write!(f, "P0"),
            Priority::P1 => write!(f, "P1"),
            Priority::P2 => write!(f, "P2"),
        }
    }
}

/// A prioritized tile in the queue.
#[derive(Debug, Clone)]
pub struct PrioritizedTile {
    pub id: String,
    pub question: String,
    pub answer: String,
    pub priority: Priority,
    pub urgency: u32,        // 0 = normal, higher = more urgent within level
    pub sequence: u64,       // insertion order for FIFO within same priority
    pub domain: String,
}

/// Priority queue stats.
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub enqueued: u64,
    pub dequeued: u64,
    pub p0_processed: u64,
    pub p1_processed: u64,
    pub p2_processed: u64,
    pub skipped_p2: u64,     // skipped because P0/P1 pending
    pub reprioritized: u64,
}

/// Deadband priority queue. P0 tiles always dequeue first.
pub struct TilePriorityQueue {
    queues: [Vec<PrioritizedTile>; 3],  // index 0=P0, 1=P1, 2=P2
    sequence: u64,
    stats: QueueStats,
    p0_active: bool,
}

impl TilePriorityQueue {
    pub fn new() -> Self {
        Self { queues: [Vec::new(), Vec::new(), Vec::new()], sequence: 0,
               stats: QueueStats::default(), p0_active: false }
    }

    /// Enqueue a tile at given priority.
    pub fn enqueue(&mut self, id: &str, question: &str, answer: &str, priority: Priority, urgency: u32, domain: &str) {
        let tile = PrioritizedTile {
            id: id.to_string(), question: question.to_string(),
            answer: answer.to_string(), priority, urgency,
            sequence: self.sequence, domain: domain.to_string(),
        };
        self.sequence += 1;
        let idx = match priority { Priority::P0 => 0, Priority::P1 => 1, Priority::P2 => 2 };
        self.queues[idx].push(tile);
        if priority == Priority::P0 { self.p0_active = true; }
        self.stats.enqueued += 1;
    }

    /// Dequeue highest-priority tile. Respects deadband: never skips P0 for P2.
    pub fn dequeue(&mut self) -> Option<PrioritizedTile> {
        // P0 first
        if !self.queues[0].is_empty() {
            let tile = self.queues[0].remove(0);
            self.stats.dequeued += 1;
            self.stats.p0_processed += 1;
            if self.queues[0].is_empty() { self.p0_active = false; }
            return Some(tile);
        }
        // P1 next
        if !self.queues[1].is_empty() {
            let tile = self.queues[1].remove(0);
            self.stats.dequeued += 1;
            self.stats.p1_processed += 1;
            return Some(tile);
        }
        // P2 only if no P0 active
        if !self.queues[2].is_empty() && !self.p0_active {
            let tile = self.queues[2].remove(0);
            self.stats.dequeued += 1;
            self.stats.p2_processed += 1;
            return Some(tile);
        }
        None
    }

    /// Dequeue with urgency sorting within priority level.
    pub fn dequeue_urgent(&mut self) -> Option<PrioritizedTile> {
        for level in 0..3 {
            if self.queues[level].is_empty() { continue; }
            if level == 2 && self.p0_active {
                self.stats.skipped_p2 += 1;
                continue;
            }
            // Find highest urgency
            let mut best = 0;
            for i in 1..self.queues[level].len() {
                if self.queues[level][i].urgency > self.queues[level][best].urgency {
                    best = i;
                } else if self.queues[level][i].urgency == self.queues[level][best].urgency
                       && self.queues[level][i].sequence < self.queues[level][best].sequence {
                    best = i;
                }
            }
            let tile = self.queues[level].remove(best);
            self.stats.dequeued += 1;
            match level {
                0 => { self.stats.p0_processed += 1; if self.queues[0].is_empty() { self.p0_active = false; } }
                1 => { self.stats.p1_processed += 1; }
                _ => { self.stats.p2_processed += 1; }
            }
            return Some(tile);
        }
        None
    }

    /// Peek at next tile without removing.
    pub fn peek(&self) -> Option<&PrioritizedTile> {
        if !self.queues[0].is_empty() { return self.queues[0].first(); }
        if !self.queues[1].is_empty() { return self.queues[1].first(); }
        if !self.queues[2].is_empty() && !self.p0_active { return self.queues[2].first(); }
        None
    }

    /// Reprioritize a tile (e.g., escalate P2 to P0).
    pub fn reprioritize(&mut self, id: &str, new_priority: Priority) -> bool {
        for level in 0..3 {
            if let Some(pos) = self.queues[level].iter().position(|t| t.id == id) {
                let mut tile = self.queues[level].remove(pos);
                tile.priority = new_priority;
                let idx = match new_priority { Priority::P0 => 0, Priority::P1 => 1, Priority::P2 => 2 };
                self.queues[idx].push(tile);
                if new_priority == Priority::P0 { self.p0_active = true; }
                self.stats.reprioritized += 1;
                return true;
            }
        }
        false
    }

    /// Drain all tiles at a specific priority level.
    pub fn drain_level(&mut self, priority: Priority) -> Vec<PrioritizedTile> {
        let idx = match priority { Priority::P0 => 0, Priority::P1 => 1, Priority::P2 => 2 };
        let tiles: Vec<PrioritizedTile> = self.queues[idx].drain(..).collect();
        if priority == Priority::P0 && tiles.is_empty() { self.p0_active = false; }
        tiles
    }

    /// Queue sizes per priority level.
    pub fn sizes(&self) -> (usize, usize, usize) {
        (self.queues[0].len(), self.queues[1].len(), self.queues[2].len())
    }

    /// Total queue size.
    pub fn len(&self) -> usize { self.queues.iter().map(|q| q.len()).sum() }
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    /// Stats.
    pub fn stats(&self) -> &QueueStats { &self.stats }

    /// Has pending P0 items.
    pub fn has_p0(&self) -> bool { !self.queues[0].is_empty() }
    pub fn has_p1(&self) -> bool { !self.queues[1].is_empty() }
    pub fn has_p2(&self) -> bool { !self.queues[2].is_empty() }

    /// Batch enqueue.
    pub fn enqueue_batch(&mut self, tiles: &[(String, String, String, Priority, u32, String)]) {
        for (id, q, a, p, u, d) in tiles {
            self.enqueue(id, q, a, *p, *u, d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_basic() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q1", "A1", Priority::P2, 0, "test");
        q.enqueue("b", "Q2", "A2", Priority::P2, 0, "test");
        assert_eq!(q.dequeue().unwrap().id, "a");
        assert_eq!(q.dequeue().unwrap().id, "b");
    }

    #[test]
    fn test_p0_before_p2() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q1", "A1", Priority::P2, 0, "test");
        q.enqueue("b", "Q2", "A2", Priority::P0, 0, "test");
        assert_eq!(q.dequeue().unwrap().id, "b"); // P0 first
        assert_eq!(q.dequeue().unwrap().id, "a"); // then P2
    }

    #[test]
    fn test_p0_before_p1() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q1", "A1", Priority::P1, 0, "test");
        q.enqueue("b", "Q2", "A2", Priority::P0, 0, "test");
        assert_eq!(q.dequeue().unwrap().id, "b");
    }

    #[test]
    fn test_p1_before_p2() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q1", "A1", Priority::P2, 0, "test");
        q.enqueue("b", "Q2", "A2", Priority::P1, 0, "test");
        assert_eq!(q.dequeue().unwrap().id, "b");
        assert_eq!(q.dequeue().unwrap().id, "a");
    }

    #[test]
    fn test_urgency_ordering() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("low", "Q1", "A1", Priority::P1, 1, "test");
        q.enqueue("high", "Q2", "A2", Priority::P1, 10, "test");
        q.enqueue("med", "Q3", "A3", Priority::P1, 5, "test");
        assert_eq!(q.dequeue_urgent().unwrap().id, "high");
        assert_eq!(q.dequeue_urgent().unwrap().id, "med");
        assert_eq!(q.dequeue_urgent().unwrap().id, "low");
    }

    #[test]
    fn test_urgency_tiebreaker_fifo() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("first", "Q1", "A1", Priority::P1, 5, "test");
        q.enqueue("second", "Q2", "A2", Priority::P1, 5, "test");
        assert_eq!(q.dequeue_urgent().unwrap().id, "first");
    }

    #[test]
    fn test_p2_skipped_when_p0_active() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("p0", "Q", "A", Priority::P0, 0, "test");
        // P0 active flag set — dequeue returns P0 only
        // After P0 drained, p0_active = false, P2 can proceed
        assert_eq!(q.dequeue().unwrap().id, "p0");
        // Now add P2
        q.enqueue("p2", "Q", "A", Priority::P2, 0, "test");
        assert_eq!(q.dequeue().unwrap().id, "p2");
    }

    #[test]
    fn test_reprioritize() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P2, 0, "test");
        assert!(q.reprioritize("a", Priority::P0));
        assert_eq!(q.dequeue().unwrap().id, "a");
        assert_eq!(q.stats().reprioritized, 1);
    }

    #[test]
    fn test_reprioritize_missing() {
        let mut q = TilePriorityQueue::new();
        assert!(!q.reprioritize("missing", Priority::P0));
    }

    #[test]
    fn test_drain_level() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P0, 0, "test");
        q.enqueue("b", "Q", "A", Priority::P0, 0, "test");
        q.enqueue("c", "Q", "A", Priority::P1, 0, "test");
        let drained = q.drain_level(Priority::P0);
        assert_eq!(drained.len(), 2);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn test_peek() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P1, 0, "test");
        q.enqueue("b", "Q", "A", Priority::P0, 0, "test");
        assert_eq!(q.peek().unwrap().id, "b"); // P0 peeks first
    }

    #[test]
    fn test_sizes() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P0, 0, "test");
        q.enqueue("b", "Q", "A", Priority::P1, 0, "test");
        q.enqueue("c", "Q", "A", Priority::P1, 0, "test");
        q.enqueue("d", "Q", "A", Priority::P2, 0, "test");
        assert_eq!(q.sizes(), (1, 2, 1));
    }

    #[test]
    fn test_stats() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P0, 0, "test");
        q.enqueue("b", "Q", "A", Priority::P1, 0, "test");
        q.enqueue("c", "Q", "A", Priority::P2, 0, "test");
        q.dequeue(); // P0
        q.dequeue(); // P1
        q.dequeue(); // P2
        assert_eq!(q.stats().enqueued, 3);
        assert_eq!(q.stats().dequeued, 3);
        assert_eq!(q.stats().p0_processed, 1);
        assert_eq!(q.stats().p1_processed, 1);
        assert_eq!(q.stats().p2_processed, 1);
    }

    #[test]
    fn test_batch_enqueue() {
        let mut q = TilePriorityQueue::new();
        q.enqueue_batch(&[
            ("a".to_string(), "Q".to_string(), "A".to_string(), Priority::P1, 0, "t".to_string()),
            ("b".to_string(), "Q".to_string(), "A".to_string(), Priority::P0, 0, "t".to_string()),
        ]);
        assert_eq!(q.len(), 2);
        assert_eq!(q.dequeue().unwrap().id, "b");
    }

    #[test]
    fn test_empty_dequeue() {
        let mut q = TilePriorityQueue::new();
        assert!(q.dequeue().is_none());
        assert!(q.dequeue_urgent().is_none());
    }

    #[test]
    fn test_clear_via_drain() {
        let mut q = TilePriorityQueue::new();
        q.enqueue("a", "Q", "A", Priority::P0, 0, "test");
        q.enqueue("b", "Q", "A", Priority::P1, 0, "test");
        q.enqueue("c", "Q", "A", Priority::P2, 0, "test");
        q.drain_level(Priority::P0);
        q.drain_level(Priority::P1);
        q.drain_level(Priority::P2);
        assert!(q.is_empty());
    }
}
