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

    use super::BroadcastTicker;

    #[tokio::test]
    async fn try_broadcast() {
        let max_timeout = Duration::from_millis(700);
        let min_timeout = Duration::from_millis(100);
        let mut ticker = BroadcastTicker::new(max_timeout, min_timeout);

        assert!(!ticker.try_broadcast());
        sleep(min_timeout).await;
        assert!(ticker.try_broadcast());
        assert!(!ticker.try_broadcast());
    }

    #[tokio::test]
    async fn wait_for_periodic_broadcast() {
        let max_timeout = Duration::from_millis(700);
        let min_timeout = Duration::from_millis(100);
        let mut ticker = BroadcastTicker::new(max_timeout, min_timeout);

        assert_ne!(
            timeout(2 * min_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
        assert_eq!(
            timeout(max_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
    }

    #[tokio::test]
    async fn wait_for_periodic_broadcast_after_try_broadcast_true() {
        let max_timeout = Duration::from_millis(700);
        let min_timeout = Duration::from_millis(100);
        let mut ticker = BroadcastTicker::new(max_timeout, min_timeout);

        sleep(min_timeout).await;
        assert!(ticker.try_broadcast());

        assert_ne!(
            timeout(2 * min_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
        assert_eq!(
            timeout(max_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
    }

    #[tokio::test]
    async fn wait_for_periodic_broadcast_after_try_broadcast_false() {
        let max_timeout = Duration::from_millis(700);
        let min_timeout = Duration::from_millis(100);
        let mut ticker = BroadcastTicker::new(max_timeout, min_timeout);

        assert!(!ticker.try_broadcast());

        assert_eq!(
            timeout(2 * min_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
        assert_ne!(
            timeout(2 * min_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
        assert_eq!(
            timeout(max_timeout, ticker.wait_for_periodic_broadcast()).await,
            Ok(())
        );
    }

    #[tokio::test]
    async fn try_broadcast_after_wait_for_periodic_broadcast() {
        let max_timeout = Duration::from_millis(700);
        let min_timeout = Duration::from_millis(100);
        let mut ticker = BroadcastTicker::new(max_timeout, min_timeout);

        assert_eq!(
            timeout(
                max_timeout + min_timeout,
                ticker.wait_for_periodic_broadcast()
            )
            .await,
            Ok(())
        );

        assert!(!ticker.try_tick());
        sleep(min_timeout).await;
        assert!(ticker.try_tick());
    }
}
