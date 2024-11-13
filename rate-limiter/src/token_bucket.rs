use std::{
    cmp::min,
    time::{Duration, Instant},
};

use log::trace;

use crate::LOG_TARGET;

pub trait TimeProvider {
    fn now(&mut self) -> Instant;
}

impl<F> TimeProvider for F
where
    F: FnMut() -> Instant,
{
    fn now(&mut self) -> Instant {
        self()
    }
}

#[derive(Clone, Default)]
pub struct DefaultTimeProvider;

impl TimeProvider for DefaultTimeProvider {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
}

/// Implementation of the `Token Bucket` algorithm for the purpose of rate-limiting access to some abstract resource.
#[derive(Clone)]
pub struct TokenBucket<T = DefaultTimeProvider> {
    rate_per_second: usize,
    available: usize,
    requested: usize,
    last_update: Instant,
    time_provider: T,
}

impl<T> std::fmt::Debug for TokenBucket<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenBucket")
            .field("rate_per_second", &self.rate_per_second)
            .field("available", &self.available)
            .field("requested", &self.requested)
            .field("last_update", &self.last_update)
            .finish()
    }
}

impl TokenBucket {
    /// Constructs a instance of [`TokenBucket`] with given target rate-per-second.
    pub fn new(rate_per_second: usize) -> Self {
        let mut time_provider = DefaultTimeProvider::default();
        TokenBucket {
            rate_per_second,
            available: rate_per_second,
            requested: 0,
            last_update: time_provider.now(),
            time_provider,
        }
    }
}

impl<T> TokenBucket<T>
where
    T: TimeProvider,
{
    #[cfg(test)]
    pub fn new_with_now(rate_per_second: usize, now: Instant, time_provider: T) -> Self
    where
        T: Clone,
    {
        TokenBucket {
            rate_per_second,
            available: rate_per_second,
            requested: 0,
            last_update: now,
            time_provider,
        }
    }

    fn calculate_delay(&self) -> Duration {
        if self.rate_per_second == 0 {
            return Duration::MAX;
        }
        let delay_micros = (self.requested - self.available)
            .saturating_mul(1_000_000)
            .saturating_div(self.rate_per_second);
        Duration::from_micros(delay_micros.try_into().unwrap_or(u64::MAX))
    }

    fn update_units(&mut self, now: Instant) -> usize {
        let time_since_last_update = now.duration_since(self.last_update);
        let mut new_units = time_since_last_update
            .as_micros()
            .saturating_mul(self.rate_per_second as u128)
            .saturating_div(1_000_000)
            .try_into()
            .unwrap_or(usize::MAX);
        let used_new_units = min(new_units, self.requested);
        new_units -= used_new_units;
        self.requested -= used_new_units;

        self.available = self.available.saturating_add(new_units);
        self.last_update = now;

        let used = min(self.available, self.requested);
        self.available -= used;
        self.requested -= used;
        self.available = min(self.available, self.token_limit());
        self.available
    }

    /// Calculates [Duration](time::Duration) by which we should delay next call to some governed resource in order to satisfy
    /// configured rate limit.
    pub fn rate_limit(&mut self, mut requested: usize) -> Option<Duration> {
        trace!(
            target: LOG_TARGET,
            "TokenBucket called for {} of requested bytes. Internal state: {:?}.",
            requested,
            self
        );
        if self.requested > 0 || self.available < requested {
            let now = self.time_provider.now();
            assert!(
                now >= self.last_update,
                "Provided value for `now` should be at least equal to `self.last_update`: now = {:#?} self.last_update = {:#?}.",
                now,
                self.last_update
            );

            if self.update_units(now) < requested {
                requested -= self.available;
                self.available = 0;
                self.requested = self.requested.saturating_add(requested);
                let required_delay = self.calculate_delay();
                return Some(required_delay);
            }
        }
        self.available -= requested;
        self.available = min(self.available, self.token_limit());
        None
    }

    fn token_limit(&self) -> usize {
        self.rate_per_second
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        time::{Duration, Instant},
    };

    use super::TokenBucket;

    #[test]
    fn token_bucket_sanity_check() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert_eq!(rate_limiter.rate_limit(9), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert!(rate_limiter.rate_limit(12).is_some());

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(8), None);
    }

    #[test]
    fn no_slowdown_while_within_rate_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert_eq!(rate_limiter.rate_limit(9), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(2);
        assert_eq!(rate_limiter.rate_limit(5), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(1), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(9), None);
    }

    #[test]
    fn slowdown_when_limit_reached() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now;
        assert_eq!(rate_limiter.rate_limit(10), None);

        // we should wait some time after reaching the limit
        *time_to_return.borrow_mut() = now;
        assert!(rate_limiter.rate_limit(1).is_some());

        *time_to_return.borrow_mut() = now;
        assert_eq!(
            rate_limiter.rate_limit(19),
            Some(Duration::from_secs(2)),
            "we should wait exactly 2 seconds"
        );
    }

    #[test]
    fn buildup_tokens_but_no_more_than_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(2);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(10);
        assert_eq!(rate_limiter.rate_limit(40), Some(Duration::from_secs(3)),);

        *time_to_return.borrow_mut() = now + Duration::from_secs(11);
        assert_eq!(rate_limiter.rate_limit(40), Some(Duration::from_secs(6)));
    }

    #[test]
    fn multiple_calls_buildup_wait_time() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(10), Some(Duration::from_secs(1)));

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(50), Some(Duration::from_secs(6)));
    }
}
