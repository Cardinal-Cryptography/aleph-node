use std::{
    cmp::min,
    pin::Pin,
    time::{Duration, Instant},
};

use futures::Future;
use tokio::time::Sleep;

#[derive(Clone)]
pub struct TokenBucket {
    rate: f64,
    tokens_limit: usize,
    available: usize,
    requested: usize,
    last_update: Instant,
}

impl TokenBucket {
    pub fn new(rate: f64) -> Self {
        let tokens_limit = rate as usize;
        Self {
            rate,
            tokens_limit,
            available: tokens_limit,
            requested: 0,
            last_update: Instant::now(),
        }
    }

    fn calculate_delay(&self) -> Duration {
        Duration::from_secs_f64((self.requested - self.available) as f64 / self.rate)
    }

    fn update_units(&mut self, now: Instant) -> usize {
        let time_since_last_update = now.duration_since(self.last_update);
        let new_units = (time_since_last_update.as_secs_f64() * self.rate).floor() as usize;
        self.available = self.available.saturating_add(new_units);
        self.last_update = now;

        let used = min(self.available, self.requested);
        self.available -= used;
        self.requested -= used;
        self.available = min(self.available, self.tokens_limit);
        self.available
    }

    pub fn rate_limit(
        &mut self,
        requested: usize,
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
        self.available = min(self.available, self.tokens_limit);
        None
    }
}

pub struct SleepingRateLimiter {
    rate_limiter: TokenBucket,
    sleep: Pin<Box<Sleep>>,
    finished: bool,
}

impl Clone for SleepingRateLimiter {
    fn clone(&self) -> Self {
        Self::new(self.rate_limiter.clone())
    }
}

impl SleepingRateLimiter {
    pub fn new(rate_limiter: TokenBucket) -> Self {
        Self {
            rate_limiter,
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
            finished: true,
        }
    }

    pub fn into_inner(self) -> TokenBucket {
        self.rate_limiter
    }
}

impl SleepingRateLimiter {
    fn set_sleep(&mut self, read_size: usize) -> RateLimiterTask {
        let mut now = None;
        let mut now_closure = || now.get_or_insert_with(|| Instant::now()).clone();
        let next_wait = self.rate_limiter.rate_limit(read_size, &mut now_closure);
        if let Some(next_wait) = next_wait {
            let wait_until = now_closure() + next_wait;
            self.finished = false;
            self.sleep.set(tokio::time::sleep_until(wait_until.into()));
        }
        self.current_sleep()
    }

    pub fn rate_limit(&mut self, read_size: usize) -> RateLimiterTask {
        self.set_sleep(read_size)
    }

    pub fn current_sleep(&mut self) -> RateLimiterTask {
        RateLimiterTask::new(&mut self.sleep, &mut self.finished)
    }
}

pub struct RateLimiterTask<'a> {
    rate_limiter_sleep: &'a mut Pin<Box<Sleep>>,
    finished: &'a mut bool,
}

impl<'a> RateLimiterTask<'a> {
    fn new(rate_limiter_sleep: &'a mut Pin<Box<Sleep>>, finished: &'a mut bool) -> Self {
        Self {
            rate_limiter_sleep,
            finished,
        }
    }
}

impl<'a> Future for RateLimiterTask<'a> {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if *self.finished {
            return std::task::Poll::Ready(());
        }
        if self.rate_limiter_sleep.as_mut().poll(cx).is_ready() {
            *self.finished = true;
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::TokenBucket;

    #[test]
    fn no_slowdown_while_within_rate_limit() {
        let limit_per_second = 10_f64;
        let mut rate_limiter = TokenBucket::new(limit_per_second);
        let now = Instant::now();

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
        let limit_per_second = 10_f64;
        let mut rate_limiter = TokenBucket::new(limit_per_second);
        let now = Instant::now();

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(1)),
            None
        );

        // we should wait some time after reaching the limit
        assert!(rate_limiter
            .rate_limit(1, || now + Duration::from_secs(1))
            .is_some());

        assert_eq!(
            rate_limiter.rate_limit(19, || now + Duration::from_secs(1)),
            Some(Duration::from_secs(2)),
            "we should wait exactly 2 seconds"
        );
    }

    #[test]
    fn buildup_tokens_but_no_more_than_limit() {
        let limit_per_second = 10_f64;
        let mut rate_limiter = TokenBucket::new(limit_per_second);
        let now = Instant::now();

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
        let limit_per_second = 10_f64;
        let mut rate_limiter = TokenBucket::new(limit_per_second);
        let now = Instant::now();

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            None
        );

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            Some(Duration::from_secs(1))
        );

        assert_eq!(
            rate_limiter.rate_limit(10, || now + Duration::from_secs(3)),
            Some(Duration::from_secs(2))
        );

        assert_eq!(
            rate_limiter.rate_limit(50, || now + Duration::from_secs(3)),
            Some(Duration::from_secs(7))
        );
    }
}
