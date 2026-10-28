//! Choosing the next operation: critical first, then deficit round robin.

use std::collections::VecDeque;
use std::time::Duration;

use super::bandwidth::{Bucket, Meter};
use super::{IoClass, Op, SchedulerConfig, UNSIZED_COST};

/// Weights of `High`, `Medium`, `Low` and `Background` (ARCHITECTURE.md
/// section 4.3: 50/25/15/10).
const WEIGHTS: [u64; 4] = [50, 25, 15, 10];
/// Bytes one unit of weight earns per round of the round robin.
const QUANTUM_UNIT: u64 = 4096;
/// Slot of `Background` in the weighted queues.
const BACKGROUND: usize = 3;

/// The scheduler of one device. See the [module documentation](super).
#[derive(Debug)]
pub struct Scheduler {
    depth: usize,
    inflight: usize,
    critical: VecDeque<Op>,
    weighted: [VecDeque<Op>; 4],
    /// Bytes each weighted class may still serve in its current visit.
    deficit: [u64; 4],
    /// The class the round robin visits.
    cursor: usize,
    /// Whether the visited class already received its quantum.
    visiting: bool,
    meter: Meter,
    bucket: Bucket,
}

/// The bytes an operation is charged.
fn charge(op: &Op) -> u64 {
    op.cost.max(UNSIZED_COST)
}

fn slot(class: IoClass) -> Option<usize> {
    match class {
        IoClass::Critical => None,
        IoClass::High => Some(0),
        IoClass::Medium => Some(1),
        IoClass::Low => Some(2),
        IoClass::Background => Some(BACKGROUND),
    }
}

impl Scheduler {
    /// An idle scheduler; `now` starts the background bucket, full.
    pub fn new(config: SchedulerConfig, now: Duration) -> Self {
        let meter = Meter::new();
        let bucket = Bucket::new(config.background_share, meter.estimate(), now);
        Self {
            depth: config.depth.max(1),
            inflight: 0,
            critical: VecDeque::new(),
            weighted: Default::default(),
            deficit: [0; 4],
            cursor: 0,
            visiting: false,
            meter,
            bucket,
        }
    }

    /// Queues an operation.
    pub fn submit(&mut self, op: Op) {
        match slot(op.class) {
            None => self.critical.push_back(op),
            Some(i) => self.weighted[i].push_back(op),
        }
    }

    /// The operation to start now, if one may: `None` when `depth`
    /// operations are in flight, when nothing waits, or when only
    /// `Background` waits and has no tokens.
    pub fn next(&mut self, now: Duration) -> Option<Op> {
        if self.inflight >= self.depth {
            return None;
        }
        self.bucket.refill(self.meter.estimate(), now);
        let op = match self.critical.pop_front() {
            Some(op) => op,
            None => self.pick_weighted()?,
        };
        if op.class == IoClass::Background {
            self.bucket.take(charge(&op));
        }
        self.meter.start(self.inflight == 0, now);
        self.inflight += 1;
        Some(op)
    }

    /// An operation started by [`Scheduler::next`] ended after moving
    /// `bytes`.
    pub fn complete(&mut self, bytes: u64, now: Duration) {
        self.inflight = self.inflight.saturating_sub(1);
        self.meter.complete(bytes, self.inflight == 0, now);
    }

    /// When [`Scheduler::next`] may return an operation without any
    /// completion in between: `now` if one is ready, the time the
    /// background bucket refills if only `Background` waits, `None` if
    /// nothing will be ready before a completion or a submission.
    pub fn next_wakeup(&self, now: Duration) -> Option<Duration> {
        if self.inflight >= self.depth {
            return None;
        }
        let others =
            !self.critical.is_empty() || self.weighted[..BACKGROUND].iter().any(|q| !q.is_empty());
        if others {
            return Some(now);
        }
        if self.weighted[BACKGROUND].is_empty() {
            return None;
        }
        self.bucket.ready_at(self.meter.estimate(), now)
    }

    /// Measured bandwidth of the device, in bytes per second.
    pub fn bandwidth(&self) -> u64 {
        // Saturating float-to-integer conversion: the estimate is finite.
        self.meter.estimate() as u64
    }

    /// Operations started and not completed.
    pub fn inflight(&self) -> usize {
        self.inflight
    }

    /// Removes and returns every queued operation (at shutdown); those in
    /// flight are not included.
    pub fn drain(&mut self) -> Vec<Op> {
        self.deficit = [0; 4];
        self.visiting = false;
        let mut ops: Vec<Op> = self.critical.drain(..).collect();
        for queue in &mut self.weighted {
            ops.extend(queue.drain(..));
        }
        ops
    }

    fn eligible(&self, i: usize) -> bool {
        !self.weighted[i].is_empty() && (i != BACKGROUND || self.bucket.ready())
    }

    /// Deficit round robin over the weighted classes. A class whose queue
    /// is empty keeps no credit, so it cannot return with a burst.
    fn pick_weighted(&mut self) -> Option<Op> {
        for i in 0..WEIGHTS.len() {
            if self.weighted[i].is_empty() {
                self.deficit[i] = 0;
            }
        }
        if !(0..WEIGHTS.len()).any(|i| self.eligible(i)) {
            return None;
        }
        loop {
            let i = self.cursor;
            if self.eligible(i) {
                if !self.visiting {
                    self.deficit[i] += WEIGHTS[i] * QUANTUM_UNIT;
                    self.visiting = true;
                }
                if let Some(op) = self.serve_head(i) {
                    return Some(op);
                }
            }
            self.cursor = (self.cursor + 1) % WEIGHTS.len();
            self.visiting = false;
            if self.cursor == 0 {
                self.skip_idle_rounds();
            }
        }
    }

    /// Serves the head of class `i` if its deficit covers it.
    fn serve_head(&mut self, i: usize) -> Option<Op> {
        let cost = charge(self.weighted[i].front()?);
        if cost > self.deficit[i] {
            return None;
        }
        self.deficit[i] -= cost;
        let op = self.weighted[i].pop_front();
        if self.weighted[i].is_empty() {
            self.deficit[i] = 0;
            self.cursor = (i + 1) % WEIGHTS.len();
            self.visiting = false;
        }
        op
    }

    /// After a full round where no class could serve its head, adds at once
    /// the rounds that would pass before any could: the same order as
    /// visiting them one by one, without looping over large operations.
    fn skip_idle_rounds(&mut self) {
        let rounds = (0..WEIGHTS.len())
            .filter(|&i| self.eligible(i))
            .filter_map(|i| {
                let head = charge(self.weighted[i].front()?);
                let missing = head.saturating_sub(self.deficit[i]);
                Some(missing.div_ceil(WEIGHTS[i] * QUANTUM_UNIT))
            })
            .min()
            .unwrap_or(0)
            .saturating_sub(1);
        let eligible: [bool; 4] = std::array::from_fn(|i| self.eligible(i));
        for ((deficit, weight), eligible) in self.deficit.iter_mut().zip(WEIGHTS).zip(eligible) {
            if eligible {
                *deficit = deficit.saturating_add(rounds.saturating_mul(weight * QUANTUM_UNIT));
            }
        }
    }
}
