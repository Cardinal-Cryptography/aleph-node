use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{Future, FutureExt};
use tokio::time::Sleep;

use crate::token_bucket::TokenBucket;

pub struct SleepingRateLimiter {
    rate_limiter: TokenBucket,
    sleep: Pin<Box<Sleep>>,
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
        }
    }

    fn set_sleep(&mut self, read_size: u64) {
        let mut now = None;
        let mut now_closure = || *now.get_or_insert_with(Instant::now);
        let next_wait = self.rate_limiter.rate_limit(read_size, &mut now_closure);
        if let Some(next_wait) = next_wait {
            let wait_until = now_closure() + next_wait;
            self.sleep.set(tokio::time::sleep_until(wait_until.into()));
        }
    }

    fn current_sleep(&mut self) -> &mut Pin<Box<Sleep>> {
        &mut self.sleep
    }

    pub fn rate_limit(mut self, read_size: u64) -> RateLimiterTask {
        self.set_sleep(read_size);
        RateLimiterTask::new(self)
    }
}

pub struct RateLimiterTask {
    sleeping_rate_limiter: Option<SleepingRateLimiter>,
}

impl RateLimiterTask {
    pub fn new(sleeping_rate_limiter: SleepingRateLimiter) -> Self {
        Self {
            sleeping_rate_limiter: Some(sleeping_rate_limiter),
        }
    }
}

impl Future for RateLimiterTask {
    type Output = SleepingRateLimiter;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.sleeping_rate_limiter.take() {
            Some(mut sleeping_rate_limiter) => {
                match sleeping_rate_limiter.current_sleep().poll_unpin(cx) {
                    std::task::Poll::Ready(_) => std::task::Poll::Ready(sleeping_rate_limiter),
                    std::task::Poll::Pending => {
                        self.sleeping_rate_limiter = Some(sleeping_rate_limiter);
                        std::task::Poll::Pending
                    }
                }
            }
            None => std::task::Poll::Pending,
        }
    }
}
