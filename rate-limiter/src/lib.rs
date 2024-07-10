mod rate_limiter;
mod token_bucket;

use tokio::io::AsyncRead;

pub use crate::rate_limiter::{FuturesRateLimiter, RateLimiter, SleepingRateLimiter};
pub use token_bucket::TokenBucket;

const LOG_TARGET: &str = "rate-limiter";

pub struct RateLimitedAsyncRead<Read> {
    rate_limiter: RateLimiter,
    inner: Read,
}

impl<Read> RateLimitedAsyncRead<Read> {
    pub fn new(read: Read, rate_limiter: RateLimiter) -> Self {
        Self {
            rate_limiter,
            inner: read,
        }
    }

    pub fn inner(&self) -> &Read {
        &self.inner
    }
}

impl<Read: AsyncRead + Unpin> AsyncRead for RateLimitedAsyncRead<Read> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let read = std::pin::Pin::new(&mut this.inner);
        this.rate_limiter.rate_limit(read, cx, buf)
    }
}

pub struct FuturesRateLimitedAsyncReadWrite<ReadWrite> {
    rate_limiter: FuturesRateLimiter,
    inner: ReadWrite,
}

impl<ReadWrite> FuturesRateLimitedAsyncReadWrite<ReadWrite> {
    pub fn new(wrapped: ReadWrite, rate_limiter: FuturesRateLimiter) -> Self {
        Self {
            rate_limiter,
            inner: wrapped,
        }
    }

    fn get_inner(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut ReadWrite>
    where
        ReadWrite: Unpin,
    {
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.inner)
    }
}

impl<Read: futures::AsyncRead + Unpin> futures::AsyncRead
    for FuturesRateLimitedAsyncReadWrite<Read>
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let read = std::pin::Pin::new(&mut this.inner);
        this.rate_limiter.rate_limit(read, cx, buf)
    }
}

impl<Write: futures::AsyncWrite + Unpin> futures::AsyncWrite
    for FuturesRateLimitedAsyncReadWrite<Write>
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.get_inner().poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_inner().poll_flush(cx)
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.get_inner().poll_close(cx)
    }
}
