use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{Future, FutureExt};
use log::trace;
use tokio::{io::AsyncRead, time::Sleep};

use crate::{token_bucket::TokenBucket, LOG_TARGET};

/// Allows to limit access to some resource. Given a preferred rate (units of something) and last used amount of units of some
/// resource, it calculates how long we should delay our next access to that resource in order to satisfy that rate.
pub struct SleepingRateLimiter {
    rate_limiter: TokenBucket,
    sleep: Pin<Box<Sleep>>,
}

impl Clone for SleepingRateLimiter {
    fn clone(&self) -> Self {
        Self {
            rate_limiter: self.rate_limiter.clone(),
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
        }
    }
}

impl SleepingRateLimiter {
    /// Constructs a instance of [SleepingRateLimiter] with given target rate-per-second.
    pub fn new(rate_per_second: usize) -> Self {
        Self {
            rate_limiter: TokenBucket::new(rate_per_second),
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
        }
    }

    fn set_sleep(&mut self, read_size: usize) -> Option<&mut Pin<Box<Sleep>>> {
        let now = Instant::now();
        let next_wait = self.rate_limiter.rate_limit(read_size, now);
        if let Some(next_wait) = next_wait {
            let wait_until = now + next_wait;
            self.sleep.set(tokio::time::sleep_until(wait_until.into()));
            trace!(
                target: LOG_TARGET,
                "Rate-Limiter will sleep until {:?} after reading {} byte(s).",
                wait_until,
                read_size
            );
            Some(&mut self.sleep)
        } else {
            None
        }
    }

    /// Given `read_size`, that is an amount of units of some governed resource, delays return of `Self` to satisfy configure
    /// rate.
    pub async fn rate_limit(mut self, read_size: usize) -> Self {
        trace!(
            target: LOG_TARGET,
            "Rate-Limiter attempting to read {}.",
            read_size
        );
        if let Some(sleep) = self.set_sleep(read_size) {
            sleep.await;
        }
        self
    }
}

type SleepFuture = impl Future<Output = SleepingRateLimiter>;

/// Wrapper around [SleepingRateLimiter] to simplify implementation of the [AsyncRead](tokio::io::AsyncRead) trait.
pub struct RateLimiter {
    rate_limiter: Pin<Box<SleepFuture>>,
}

impl RateLimiter {
    /// Constructs an instance of [RateLimiter] that uses already configured rate-limiting access governor
    /// ([SleepingRateLimiter]).
    pub fn new(rate_limiter: SleepingRateLimiter) -> Self {
        Self {
            rate_limiter: Box::pin(rate_limiter.rate_limit(0)),
        }
    }

    /// Helper method for the use of the [AsyncRead](tokio::io::AsyncRead) implementation.
    pub fn rate_limit<Read: AsyncRead + Unpin>(
        &mut self,
        read: std::pin::Pin<&mut Read>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let sleeping_rate_limiter = match self.rate_limiter.poll_unpin(cx) {
            std::task::Poll::Ready(rate_limiter) => rate_limiter,
            _ => return std::task::Poll::Pending,
        };

        let filled_before = buf.filled().len();
        let result = read.poll_read(cx, buf);
        let filled_after = buf.filled().len();
        let last_read_size = filled_after.saturating_sub(filled_before);

        self.rate_limiter
            .set(sleeping_rate_limiter.rate_limit(last_read_size));

        result
    }
}
