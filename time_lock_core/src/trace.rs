#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Event {
    Startup,
    SeedInput,
    SeedObserved,
    PreviewCommand,
    ClockFault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    pub event: Event,
    pub session: u32,
    pub tick: u64,
    pub real_ms: i64,
    pub game_ms: i64,
    pub value: u64,
}

impl Observation {
    const EMPTY: Self = Self {
        event: Event::Startup,
        session: 0,
        tick: 0,
        real_ms: 0,
        game_ms: 0,
        value: 0,
    };
}

/// Preserve the first reads. Never allocate or perform IO in a game hook.
#[derive(Debug, Clone, Copy)]
pub struct Trace<const N: usize> {
    entries: [Observation; N],
    len: usize,
    dropped: u32,
}

impl<const N: usize> Default for Trace<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Trace<N> {
    pub const fn new() -> Self {
        Self {
            entries: [Observation::EMPTY; N],
            len: 0,
            dropped: 0,
        }
    }
    pub fn push(&mut self, event: Observation) {
        if self.len < N {
            self.entries[self.len] = event;
            self.len += 1;
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }
    pub fn entries(&self) -> &[Observation] {
        &self.entries[..self.len]
    }
    pub fn dropped(&self) -> u32 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_observation_survives_overflow() {
        let mut trace = Trace::<2>::new();
        for value in 0..10 {
            trace.push(Observation {
                value,
                ..Observation::EMPTY
            });
        }
        assert_eq!(trace.entries().len(), 2);
        assert_eq!(trace.entries()[0].value, 0);
        assert_eq!(trace.entries()[1].value, 1);
        assert_eq!(trace.dropped(), 8);
        let mut empty = Trace::<0>::new();
        empty.push(Observation::EMPTY);
        assert_eq!(empty.dropped(), 1);
    }
}
