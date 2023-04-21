use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::Future;
use tokio::time::Sleep;

use crate::token_bucket::TokenBucket;

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
        let mut now_closure = || *now.get_or_insert_with(Instant::now);
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
