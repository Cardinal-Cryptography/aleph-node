use tokio::time::{sleep, Duration, Instant};

/// This struct is used for rate limiting as an on-demand ticker. It can be used for ticking
/// at least once `max_timeout` but not more than once every `min_timeout`.
pub struct Ticker {
    last_tick: Instant,
    current_timeout: Duration,
    max_timeout: Duration,
    min_timeout: Duration,
}

impl Ticker {
    pub fn new(max_timeout: Duration, min_timeout: Duration) -> Self {
        Self {
            last_tick: Instant::now(),
            current_timeout: max_timeout,
            max_timeout,
            min_timeout,
        }
    }

    /// Returns whether at least `min_timeout` time elapsed since last tick.
    /// If `min_timeout` elapsed since last tick, returns true, sets last tick to now,
    /// current timout to `max_timeout` and will Return true again if called after `min_timeout`.
    /// If not, returns false and sets current timeout to `min_timeout`.
    pub fn try_tick(&mut self) -> bool {
        let now = Instant::now();
        if now.saturating_duration_since(self.last_tick) >= self.min_timeout {
            self.last_tick = now;
            self.current_timeout = self.min_timeout;
            true
        } else {
            self.current_timeout = self.min_timeout;
            false
        }
    }

    /// Sleeps until next tick should happen. In case enough time elapsed,
    /// sets last tick to now and current timeout to `max_timeout`.
    pub async fn wait(&mut self) {
        let since_last = Instant::now().saturating_duration_since(self.last_tick);
        sleep(self.current_timeout.saturating_sub(since_last)).await;
        self.current_timeout = self.max_timeout;
        self.last_tick = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::{sleep, timeout, Duration};

    use super::Ticker;

    const MAX_TIMEOUT: Duration = Duration::from_millis(700);
    const MIN_TIMEOUT: Duration = Duration::from_millis(100);

    const MAX_TIMEOUT_PLUS: Duration = Duration::from_millis(800);
    const MIN_TIMEOUT_PLUS: Duration = Duration::from_millis(200);

    fn setup_ticker() -> Ticker {
        Ticker::new(MAX_TIMEOUT, MIN_TIMEOUT)
    }

    #[tokio::test]
    async fn try_tick() {
        let mut ticker = setup_ticker();

        assert!(!ticker.try_tick());
        sleep(MIN_TIMEOUT).await;
        assert!(ticker.try_tick());
        assert!(!ticker.try_tick());
    }

    #[tokio::test]
    async fn wait() {
        let mut ticker = setup_ticker();

        assert_ne!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MAX_TIMEOUT, ticker.wait()).await, Ok(()));
    }

    #[tokio::test]
    async fn wait_after_try_tick_true() {
        let mut ticker = setup_ticker();

        assert!(!ticker.try_tick());
        sleep(MIN_TIMEOUT).await;
        assert!(ticker.try_tick());

        assert_ne!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MAX_TIMEOUT, ticker.wait()).await, Ok(()));
    }

    #[tokio::test]
    async fn wait_after_try_tick_false() {
        let mut ticker = setup_ticker();

        assert!(!ticker.try_tick());

        assert_eq!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_ne!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MAX_TIMEOUT, ticker.wait()).await, Ok(()));
    }

    #[tokio::test]
    async fn try_tick_after_wait() {
        let mut ticker = setup_ticker();

        assert_eq!(timeout(MAX_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));

        assert!(!ticker.try_tick());
        sleep(MIN_TIMEOUT).await;
        assert!(ticker.try_tick());
    }
}
