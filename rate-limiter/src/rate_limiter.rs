use std::{
    pin::Pin,
    time::{Duration, Instant},
};

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

    fn set_sleep(&mut self, read_size: u64) -> &mut Pin<Box<Sleep>> {
        let mut now = None;
        let mut now_closure = || *now.get_or_insert_with(Instant::now);
        let next_wait = self.rate_limiter.rate_limit(read_size, &mut now_closure);
        if let Some(next_wait) = next_wait {
            let wait_until = now_closure() + next_wait;
            self.sleep.set(tokio::time::sleep_until(wait_until.into()));
        }
        &mut self.sleep
    }

    pub async fn rate_limit(mut self, read_size: u64) -> Self {
        self.set_sleep(read_size).await;
        self
    }
}
