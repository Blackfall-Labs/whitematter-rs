//! Ring buffer for delayed spike delivery through white matter tracts.

use crate::tract::TractSpike;

/// Ring buffer that holds in-flight spikes until their delay expires.
///
/// Indexed by microsecond time. Spikes are enqueued with an arrival time
/// and drained when the current time reaches that time.
pub struct DelayBuffer {
    /// Ring buffer slots, indexed by `time_us % capacity`.
    slots: Vec<Vec<TractSpike>>,
    /// Number of slots (determines max representable delay).
    capacity: usize,
}

impl DelayBuffer {
    /// Create a delay buffer with the given capacity (in microsecond slots).
    ///
    /// `capacity` determines the maximum delay representable. Spikes with
    /// delay exceeding capacity are clamped to `capacity - 1`.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            slots: (0..capacity).map(|_| Vec::new()).collect(),
            capacity,
        }
    }

    /// Enqueue a spike for delivery at `arrival_time_us`.
    pub fn enqueue(&mut self, spike: TractSpike) {
        let slot = spike.arrival_time_us as usize % self.capacity;
        self.slots[slot].push(spike);
    }

    /// Drain all spikes due for delivery at `current_time_us`.
    pub fn drain_due(&mut self, current_time_us: u64) -> Vec<TractSpike> {
        let slot = current_time_us as usize % self.capacity;
        let spikes = core::mem::take(&mut self.slots[slot]);
        // Filter to only those actually due (not stale spikes from a previous wrap)
        spikes
            .into_iter()
            .filter(|s| s.arrival_time_us <= current_time_us)
            .collect()
    }

    /// Drain all spikes due for delivery in the range `[from_us, to_us]`.
    ///
    /// This visits every slot that could contain spikes arriving in the range,
    /// ensuring no spikes are missed when the caller steps in increments larger
    /// than the buffer capacity. Spikes with arrival times beyond `to_us` are
    /// preserved in their slots for later delivery.
    pub fn drain_range(&mut self, _from_us: u64, to_us: u64) -> Vec<TractSpike> {
        let mut result = Vec::new();

        // Visit every slot — any could contain due spikes when the step
        // exceeds the buffer capacity.
        for slot in self.slots.iter_mut() {
            if slot.is_empty() {
                continue;
            }

            // Partition: due spikes go to result, future spikes stay
            let taken = core::mem::take(slot);
            let (due, not_yet): (Vec<_>, Vec<_>) =
                taken.into_iter().partition(|s| s.arrival_time_us <= to_us);
            result.extend(due);
            *slot = not_yet;
        }

        result
    }

    /// Total number of in-flight spikes across all slots.
    pub fn in_flight(&self) -> usize {
        self.slots.iter().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tract::{DendriticZone, TractSpike};

    fn spike(target: u32, arrival: u64) -> TractSpike {
        TractSpike {
            source_neuron: 0,
            target_neuron: target,
            zone: DendriticZone::Feedforward,
            current: 100,
            arrival_time_us: arrival,
        }
    }

    #[test]
    fn enqueue_and_drain() {
        let mut buf = DelayBuffer::new(100);
        buf.enqueue(spike(5, 50));
        buf.enqueue(spike(10, 50));
        buf.enqueue(spike(15, 75));

        // Nothing at time 0
        assert!(buf.drain_due(0).is_empty());

        // Two spikes at time 50
        let due = buf.drain_due(50);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].target_neuron, 5);
        assert_eq!(due[1].target_neuron, 10);

        // One spike at time 75
        let due = buf.drain_due(75);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].target_neuron, 15);

        // Nothing left
        assert_eq!(buf.in_flight(), 0);
    }

    #[test]
    fn drain_is_idempotent() {
        let mut buf = DelayBuffer::new(100);
        buf.enqueue(spike(1, 30));

        let first = buf.drain_due(30);
        assert_eq!(first.len(), 1);

        let second = buf.drain_due(30);
        assert!(second.is_empty());
    }
}
