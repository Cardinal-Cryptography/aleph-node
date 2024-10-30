use rate_limiter::{SharingRateLimiter, RateLimitedAsyncRead, RateLimiterImpl};

use crate::{ConnectionInfo, Data, Dialer, Listener, PeerAddressInfo, Splittable, Splitted};

impl<Read: ConnectionInfo> ConnectionInfo for RateLimitedAsyncRead<Read> {
    fn peer_address_info(&self) -> PeerAddressInfo {
        self.inner().peer_address_info()
    }
}

/// Implementation of the [Dialer] trait governing all returned [Dialer::Connection] instances by a rate-limiting wrapper.
#[derive(Clone)]
pub struct RateLimitingDialer<D> {
    dialer: D,
    rate_limiter: SharingRateLimiter,
}

impl<D> RateLimitingDialer<D> {
    pub fn new(dialer: D, rate_limiter: SharingRateLimiter) -> Self {
        Self {
            dialer,
            rate_limiter,
        }
    }
}

#[async_trait::async_trait]
impl<A, D> Dialer<A> for RateLimitingDialer<D>
where
    A: Data,
    D: Dialer<A>,
    <D::Connection as Splittable>::Sender: Unpin,
    <D::Connection as Splittable>::Receiver: Unpin,
{
    type Connection = Splitted<
        RateLimitedAsyncRead<<D::Connection as Splittable>::Receiver>,
        <D::Connection as Splittable>::Sender,
    >;
    type Error = D::Error;

    async fn connect(&mut self, address: A) -> Result<Self::Connection, Self::Error> {
        let connection = self.dialer.connect(address).await?;
        let (sender, receiver) = connection.split();
        Ok(Splitted(
            RateLimitedAsyncRead::new(receiver, RateLimiterImpl::new(self.rate_limiter.clone())),
            sender,
        ))
    }
}

/// Implementation of the [Listener] trait governing all returned [Listener::Connection] instances by a rate-limiting wrapper.
pub struct RateLimitingListener<L> {
    listener: L,
    rate_limiter: SharingRateLimiter,
}

impl<L> RateLimitingListener<L> {
    pub fn new(listener: L, rate_limiter: SharingRateLimiter) -> Self {
        Self {
            listener,
            rate_limiter,
        }
    }
}

#[async_trait::async_trait]
impl<L> Listener for RateLimitingListener<L>
where
    L: Listener + Send,
{
    type Connection = Splitted<
        RateLimitedAsyncRead<<L::Connection as Splittable>::Receiver>,
        <L::Connection as Splittable>::Sender,
    >;
    type Error = L::Error;

    async fn accept(&mut self) -> Result<Self::Connection, Self::Error> {
        let connection = self.listener.accept().await?;
        let (sender, receiver) = connection.split();
        Ok(Splitted(
            RateLimitedAsyncRead::new(receiver, RateLimiterImpl::new(self.rate_limiter.clone())),
            sender,
        ))
    }
}
