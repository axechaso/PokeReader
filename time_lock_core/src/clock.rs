use crate::date::{check_ms, DateError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Disabled,
    Frozen,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    Date(DateError),
    Disabled,
    NotFrozen,
    InvalidRate,
    TickRegression,
    Overflow,
}

impl From<DateError> for ClockError {
    fn from(value: DateError) -> Self {
        Self::Date(value)
    }
}

/// All timing comes from the caller. No OS clock, RNG or global state is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    mode: Mode,
    anchor_ms: i64,
    anchor_tick: u64,
    last_tick: u64,
    tick_hz: u32,
    session: u32,
}

impl Clock {
    pub const fn new(tick_hz: u32) -> Result<Self, ClockError> {
        if tick_hz == 0 {
            return Err(ClockError::InvalidRate);
        }
        Ok(Self {
            mode: Mode::Disabled,
            anchor_ms: 0,
            anchor_tick: 0,
            last_tick: 0,
            tick_hz,
            session: 0,
        })
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn session(&self) -> u32 {
        self.session
    }

    pub fn start_session(
        &mut self,
        target_ms: i64,
        mode: Mode,
        tick: u64,
    ) -> Result<(), ClockError> {
        check_ms(target_ms)?;
        if mode == Mode::Disabled {
            return Err(ClockError::Disabled);
        }
        let session = self.session.checked_add(1).ok_or(ClockError::Overflow)?;
        self.mode = mode;
        self.anchor_ms = target_ms;
        self.anchor_tick = tick;
        self.last_tick = tick;
        self.session = session;
        Ok(())
    }

    pub fn read(&mut self, tick: u64) -> Result<i64, ClockError> {
        match self.mode {
            Mode::Disabled => Err(ClockError::Disabled),
            Mode::Frozen => Ok(self.anchor_ms),
            Mode::Running => {
                if tick < self.last_tick {
                    return Err(ClockError::TickRegression);
                }
                let delta = tick
                    .checked_sub(self.anchor_tick)
                    .ok_or(ClockError::TickRegression)?;
                let hz = u64::from(self.tick_hz);
                // Divide before multiplying: works even near u64::MAX ticks.
                let whole = (delta / hz).checked_mul(1000).ok_or(ClockError::Overflow)?;
                let ms = whole
                    .checked_add((delta % hz) * 1000 / hz)
                    .ok_or(ClockError::Overflow)?;
                let ms = i64::try_from(ms).map_err(|_| ClockError::Overflow)?;
                let value = check_ms(self.anchor_ms.checked_add(ms).ok_or(ClockError::Overflow)?)?;
                self.last_tick = tick;
                Ok(value)
            }
        }
    }

    pub fn freeze(&mut self, tick: u64) -> Result<i64, ClockError> {
        let value = self.read(tick)?;
        self.anchor_ms = value;
        self.anchor_tick = tick;
        self.last_tick = tick;
        self.mode = Mode::Frozen;
        Ok(value)
    }

    pub fn release(&mut self, tick: u64) -> Result<i64, ClockError> {
        let value = self.read(tick)?;
        if self.mode == Mode::Frozen {
            self.anchor_tick = tick;
            self.last_tick = tick;
            self.mode = Mode::Running;
        }
        Ok(value)
    }

    pub fn step(&mut self, delta_ms: i64) -> Result<i64, ClockError> {
        if self.mode != Mode::Frozen {
            return Err(ClockError::NotFrozen);
        }
        let value = check_ms(
            self.anchor_ms
                .checked_add(delta_ms)
                .ok_or(ClockError::Overflow)?,
        )?;
        self.anchor_ms = value;
        Ok(value)
    }

    pub fn end_session(&mut self) {
        self.mode = Mode::Disabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::MAX_MS;

    #[test]
    fn freeze_release_step_and_retry() {
        let mut c = Clock::new(268_123_480).unwrap();
        assert_eq!(c.read(0), Err(ClockError::Disabled));
        c.start_session(1000, Mode::Frozen, 100).unwrap();
        assert_eq!(c.read(u64::MAX), Ok(1000));
        assert_eq!(c.read(0), Ok(1000));
        assert_eq!(c.step(-1), Ok(999));
        assert_eq!(c.release(500), Ok(999));
        assert_eq!(c.read(268_123_980), Ok(1999));
        assert_eq!(c.freeze(536_247_460), Ok(2999));
        assert_eq!(c.read(u64::MAX), Ok(2999));
        c.end_session();
        assert_eq!(c.step(1), Err(ClockError::NotFrozen));
        c.start_session(1000, Mode::Frozen, 0).unwrap();
        assert_eq!(c.read(0), Ok(1000));
        assert_eq!(c.session(), 2);
    }

    #[test]
    fn repeat_release_preserves_fractional_ticks() {
        let mut c = Clock::new(10_000).unwrap();
        c.start_session(0, Mode::Running, 0).unwrap();
        for tick in 1..100 {
            c.release(tick).unwrap();
        }
        assert_eq!(c.read(100), Ok(10));
    }

    #[test]
    fn failed_commands_leave_state_unchanged() {
        let mut c = Clock::new(1000).unwrap();
        c.start_session(0, Mode::Frozen, 0).unwrap();
        let old = c;
        assert!(c.step(-1).is_err());
        assert_eq!(c, old);
        assert!(c.start_session(MAX_MS + 1, Mode::Running, 1).is_err());
        assert_eq!(c, old);
        c.release(0).unwrap();
        c.read(100).unwrap();
        let old = c;
        assert_eq!(c.read(99), Err(ClockError::TickRegression));
        assert!(c.step(1).is_err());
        assert_eq!(c, old);
        assert!(Clock::new(0).is_err());
    }

    #[test]
    fn long_duration_does_not_multiply_ticks_first() {
        let mut c = Clock::new(268_123_480).unwrap();
        c.start_session(0, Mode::Running, 0).unwrap();
        let ticks = 268_123_480u64 * 86_400 * 36_524;
        assert!(ticks.checked_mul(1000).is_none());
        assert_eq!(c.read(ticks), Ok(36_524 * 86_400_000));
        let old = c;
        assert!(c.read(u64::MAX).is_err());
        assert_eq!(c, old);
    }

    #[test]
    fn milliseconds_cross_day_and_year() {
        let mut c = Clock::new(1000).unwrap();
        c.start_session(86_399_999, Mode::Frozen, 0).unwrap();
        assert_eq!(c.step(1), Ok(86_400_000));
        c.start_session(MAX_MS, Mode::Frozen, 0).unwrap();
        let old = c;
        assert!(c.step(1).is_err());
        assert_eq!(c, old);
        assert_eq!(c.step(-60_000), Ok(MAX_MS - 60_000));
    }
}
