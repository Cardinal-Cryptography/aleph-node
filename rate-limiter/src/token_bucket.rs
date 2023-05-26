use std::{
    cmp::min,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct TokenBucket {
    rate_per_second: u64,
    available: u64,
    requested: u64,
    last_update: Instant,
}

impl TokenBucket {
    pub fn new(rate_per_second: u64) -> Self {
        Self {
            rate_per_second,
            available: rate_per_second,
            requested: 0,
            last_update: Instant::now(),
        }
    }

    #[cfg(test)]
    pub fn new_with_now(rate_per_second: u64, now: Instant) -> Self {
        Self {
            last_update: now,
            ..Self::new(rate_per_second)
        }
    }

    fn calculate_delay(&self) -> Duration {
        let delay_micros = (self.requested - self.available)
            .saturating_mul(1_000_000)
            .saturating_div(self.rate_per_second);
        Duration::from_micros(delay_micros)
    }

    fn update_units(&mut self, now: Instant) -> u64 {
        let time_since_last_update = now.duration_since(self.last_update);
        let new_units = time_since_last_update
            .as_micros()
            .saturating_mul(self.rate_per_second as u128)
            .saturating_div(1_000_000)
            .try_into()
            .unwrap_or(u64::MAX);
        self.available = self.available.saturating_add(new_units);
        self.last_update = now;

        let used = min(self.available, self.requested);
        self.available -= used;
        self.requested -= used;
        self.available = min(self.available, self.token_limit());
        self.available
    }

    pub fn rate_limit(
        &mut self,
        requested: u64,
        mut now: impl FnMut() -> Instant,
    ) -> Option<Duration> {
        if self.requested > 0 || self.available < requested {
            let now_value = now();
            assert!(
                now_value >= self.last_update,
                "Provided value for `now` should be at least equal to `self.last_update`: now = {:#?} self.last_update = {:#?}.",
                now_value,
                self.last_update
            );
            if self.update_units(now_value) < requested {
                self.requested = self.requested.saturating_add(requested);
                let required_delay = self.calculate_delay();
                return Some(required_delay);
            }
        }
        self.available -= requested;
        self.available = min(self.available, self.token_limit());
        None
    }

    fn token_limit(&self) -> u64 {
        self.rate_per_second
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::TokenBucket;

    #[test]
    fn token_bucket_sanity_check() {
        let limit_per_second = 10;
        let now = Instant::now();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now);

        assert_eq!(
            rate_limiter.rate_limit(9, || now + Duration::from_secs(1)),
            None
        );

        assert!(rate_limiter
            .rate_limit(12, || now + Duration::from_secs(1))
            .is_some());

        assert_eq!(
            rate_limiter.rate_limit(8, || now + Duration::from_secs(3)),
            None
        );
    }

    #[test]
    fn no_slowdown_while_within_rate_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now);

        assert_eq!(
            rate_limiter.rate_limit(9, || now + Duration::from_secs(1)),
            None
        );
        assert_eq!(
            rate_limiter.rate_limit(5, || now + Duration::from_secs(2)),
            None
        );
        assert_eq!(
            rate_limiter.rate_limit(1, || now + Duration::from_secs(3)),
            None
        );
        assert_eq!(
            rate_limiter.rate_limit(0, || panic!("`now()` shouldn't be called")),
            None
        );
        assert_eq!(
            rate_limiter.rate_limit(0, || panic!("`now()` shouldn't be called")),
            None
        );
        assert_eq!(
            rate_limiter.rate_limit(9, || now + Duration::from_secs(3)),
            None
        );
    }

    #[test]
    fn slowdown_when_limit_reached() {
        let limit_per_second = 10;
        let now = Instant::now();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now);

        assert_eq!(rate_limiter.rate_limit(10, || now), None);

        // we should wait some time after reaching the limit
        assert!(rate_limiter.rate_limit(1, || now).is_some());

        assert_eq!(
            rate_limiter.rate_limit(19, || now),
            Some(Duration::from_secs(2)),
            "we should wait exactly 2 seconds"
        );
    }

    #[test]
    fn buildup_tokens_but_no_more_than_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now);

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(2)),
            None
        );

        assert_eq!(
            rate_limiter.rate_limit(40, || now + Duration::from_secs(10)),
            Some(Duration::from_secs(3)),
        );
        assert_eq!(
            rate_limiter.rate_limit(40, || now + Duration::from_secs(11)),
            Some(Duration::from_secs(6))
        );
    }

    #[test]
    fn multiple_calls_buildup_wait_time() {
        let limit_per_second = 10;
        let now = Instant::now();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now);

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            None
        );

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            None
        );

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            Some(Duration::from_secs(1))
        );

        assert_eq!(
            rate_limiter.rate_limit(50, || now + Duration::from_secs(3)),
            Some(Duration::from_secs(6))
        );
    }
}