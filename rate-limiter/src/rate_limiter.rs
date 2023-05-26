use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{Future, FutureExt};
use tokio::{io::AsyncRead, time::Sleep};

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

type SleepFuture = impl Future<Output = SleepingRateLimiter>;

pub struct RateLimitedAsyncRead<A> {
    rate_limiter: Pin<Box<SleepFuture>>,
    read: A,
}

impl<A> RateLimitedAsyncRead<A> {
    pub fn new(read: A, rate_limiter: SleepingRateLimiter) -> Self {
        Self {
            rate_limiter: Box::pin(rate_limiter.rate_limit(0)),
            read,
        }
    }

    pub fn inner(&self) -> &A {
        &self.read
    }
}

impl<A: AsyncRead + Unpin> AsyncRead for RateLimitedAsyncRead<A> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let sleeping_rate_limiter = match self.rate_limiter.poll_unpin(cx) {
            std::task::Poll::Ready(rate_limiter) => rate_limiter,
            _ => return std::task::Poll::Pending,
        };

        let filled_before = buf.filled().len();
        let result = Pin::new(&mut self.read).poll_read(cx, buf);
        let filled_after = buf.filled().len();
        let last_read_size = filled_after - filled_before;
        let last_read_size = last_read_size.try_into().unwrap_or(u64::MAX);

        self.rate_limiter
            .set(sleeping_rate_limiter.rate_limit(last_read_size));

        result
    }
}
