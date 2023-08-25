use tokio::time::{sleep, Duration, Instant};

/// This struct is used for rate limiting as an on-demand ticker. It can be used for ticking
/// at most after `max_timeout` but no sooner than after `min_timeout`.
/// Example usage would be to use the `wait` method in main select loop and
/// `try_tick` whenever you would like to tick sooner in another branch of select,
/// resetting whenever the rate limited action actually occurs.
pub struct Ticker {
    last_reset: Instant,
    current_timeout: Duration,
    max_timeout: Duration,
    min_timeout: Duration,
}

impl Ticker {
    /// Returns new Ticker struct. Enforces `max_timeout` >= `min_timeout`.
    pub fn new(mut max_timeout: Duration, min_timeout: Duration) -> Self {
        if max_timeout < min_timeout {
            max_timeout = min_timeout;
        };
        Self {
            last_reset: Instant::now(),
            current_timeout: max_timeout,
            max_timeout,
            min_timeout,
        }
    }

    /// Returns whether at least `min_timeout` time elapsed since the last reset.
    /// If it has not, the next call to `wait` will return when `min_timeout` elapses.
    pub fn try_tick(&mut self) -> bool {
        let now = Instant::now();
        if now.saturating_duration_since(self.last_reset) >= self.min_timeout {
            self.current_timeout = self.max_timeout;
            true
        } else {
            self.current_timeout = self.min_timeout;
            false
        }
    }

    /// Sleeps until next tick should happen.
    /// Returns when enough time elapsed.
    /// Note that after `max_timeout` elapsed since the last reset this will return immediately.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe.
    pub async fn wait(&mut self) {
        self.wait_current_timeout().await;
        self.current_timeout = self.max_timeout;
    }

    /// Reset the ticker, making it time from the moment of this call.
    /// Behaves as if it was just created with the same parametres.
    pub fn reset(&mut self) {
        self.last_reset = Instant::now();
        self.current_timeout = self.max_timeout;
    }

    async fn wait_current_timeout(&self) {
        let since_last = Instant::now().saturating_duration_since(self.last_reset);
        sleep(self.current_timeout.saturating_sub(since_last)).await;
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
        sleep(MIN_TIMEOUT_PLUS).await;
        assert!(ticker.try_tick());
        assert!(ticker.try_tick());
    }

    #[tokio::test]
    async fn wait() {
        let mut ticker = setup_ticker();

        assert_ne!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MAX_TIMEOUT, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MIN_TIMEOUT, ticker.wait()).await, Ok(()));
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

        assert!(ticker.try_tick());
    }

    #[tokio::test]
    async fn wait_after_reset() {
        let mut ticker = setup_ticker();

        assert_eq!(timeout(MAX_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));

        ticker.reset();
        assert_ne!(timeout(MIN_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
        assert_eq!(timeout(MAX_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));
    }

    #[tokio::test]
    async fn try_tick_after_reset() {
        let mut ticker = setup_ticker();

        assert_eq!(timeout(MAX_TIMEOUT_PLUS, ticker.wait()).await, Ok(()));

        ticker.reset();
        assert!(!ticker.try_tick());
        sleep(MIN_TIMEOUT).await;
        assert!(ticker.try_tick());
    }
}
