use tokio::time::{sleep, Duration, Instant};

/// This struct is used for determining when we should broadcast justification so that it does not happen too often.
pub struct BroadcastTicker {
    last_broadcast: Instant,
    last_periodic_broadcast: Instant,
    current_periodic_timeout: Duration,
    max_timeout: Duration,
    min_timeout: Duration,
}

impl BroadcastTicker {
    pub fn new(max_timeout: Duration, min_timeout: Duration) -> Self {
        Self {
            last_broadcast: Instant::now(),
            last_periodic_broadcast: Instant::now(),
            current_periodic_timeout: max_timeout,
            max_timeout,
            min_timeout,
        }
    }

    /// Returns whether we should broadcast right now if we just imported a justification.
    /// If min_timeout elapsed since last broadcast, returns true, sets last broadcast to now and will
    /// return true again if called after `self.min_timeout`.
    /// If not, returns false and sets periodic broadcast timeout to `self.min_timeout`.
    /// This is to prevent from sending every justification when importing a batch of them. This way,
    /// when receiving batch of justifications we will broadcast the first justification and the highest known
    /// after `self.min_timeout` using periodic broadcast.
    pub fn try_broadcast(&mut self) -> bool {
        let now = Instant::now();
        if now.saturating_duration_since(self.last_broadcast) >= self.min_timeout {
            self.last_broadcast = now;
            true
        } else {
            self.current_periodic_timeout = self.min_timeout;
            false
        }
    }

    /// Sleeps until next periodic broadcast should happen.
    /// In case time elapsed, sets last periodic broadcast to now and periodic timeout to `self.max_timeout`.
    pub async fn wait_for_periodic_broadcast(&mut self) {
        let since_last = Instant::now().saturating_duration_since(self.last_periodic_broadcast);
        sleep(self.current_periodic_timeout.saturating_sub(since_last)).await;
        self.current_periodic_timeout = self.max_timeout;
        self.last_periodic_broadcast = Instant::now();
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
    async fn wait_for_periodic_broadcast_after_try_broadcast() {
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
}
