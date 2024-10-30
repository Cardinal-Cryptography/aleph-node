use std::{task::ready, time::Instant};

use futures::{
    future::{pending, BoxFuture},
    FutureExt,
};
use tokio::io::AsyncRead;

use crate::{token_bucket::SharedTokenBucket, RatePerSecond};

pub type SharingRateLimiter = RateLimiterFacade;

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum Deadline {
    Never,
    Instant(Instant),
}

impl From<Deadline> for Option<Instant> {
    fn from(value: Deadline) -> Self {
        match value {
            Deadline::Never => None,
            Deadline::Instant(value) => Some(value),
        }
    }
}

/// Wrapper around [RateLimiterFacade] to simplify implementation of the [AsyncRead](tokio::io::AsyncRead) trait.
pub struct RateLimiterImpl {
    rate_limiter: BoxFuture<'static, RateLimiterFacade>,
}

impl RateLimiterImpl {
    /// Constructs an instance of [RateLimiterImpl] that uses already configured rate-limiting access governor
    /// ([RateLimiterFacade]).
    pub fn new(rate_limiter: RateLimiterFacade) -> Self {
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
        let sleeping_rate_limiter = ready!(self.rate_limiter.poll_unpin(cx));

        let filled_before = buf.filled().len();
        let result = read.poll_read(cx, buf);
        let filled_after: &[u8] = buf.filled();
        let filled_after = 8 * filled_after.len();
        let last_read_size = filled_after.saturating_sub(filled_before);

        self.rate_limiter = sleeping_rate_limiter.rate_limit(last_read_size).boxed();

        result
    }
}

pub struct FuturesRateLimiter {
    rate_limiter: BoxFuture<'static, RateLimiterFacade>,
}

impl FuturesRateLimiter {
    /// Constructs an instance of [RateLimiter] that uses already configured rate-limiting access governor
    /// ([SleepingRateLimiter]).
    pub fn new(rate_limiter: RateLimiterFacade) -> Self {
        Self {
            rate_limiter: Box::pin(rate_limiter.rate_limit(0)),
        }
    }

    /// Helper method for the use of the [AsyncRead](futures::AsyncRead) implementation.
    pub fn rate_limit<Read: futures::AsyncRead + Unpin>(
        &mut self,
        read: std::pin::Pin<&mut Read>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let sleeping_rate_limiter = ready!(self.rate_limiter.poll_unpin(cx));

        let result = read.poll_read(cx, buf);
        let last_read_size = match &result {
            std::task::Poll::Ready(Ok(read_size)) => 8 * *read_size,
            _ => 0,
        };

        self.rate_limiter = sleeping_rate_limiter.rate_limit(last_read_size).boxed();

        result
    }
}

#[derive(Clone)]
pub enum RateLimiterFacade {
    NoTraffic,
    RateLimiter(SharedTokenBucket),
}

impl RateLimiterFacade {
    pub fn new(rate: RatePerSecond) -> Self {
        match rate {
            RatePerSecond::Block => Self::NoTraffic,
            RatePerSecond::Rate(rate) => Self::RateLimiter(SharedTokenBucket::new(rate)),
        }
    }

    async fn rate_limit(self, read_size: usize) -> Self {
        match self {
            RateLimiterFacade::NoTraffic => pending().await,
            RateLimiterFacade::RateLimiter(rate_limiter) => RateLimiterFacade::RateLimiter(
                rate_limiter
                    .rate_limit(read_size.try_into().unwrap_or(u64::MAX))
                    .await,
            ),
        }
    }
}
