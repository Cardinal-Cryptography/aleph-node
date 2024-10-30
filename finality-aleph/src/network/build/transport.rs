use libp2p::{core::muxing::StreamMuxer, PeerId, Transport};
use rate_limiter::{FuturesRateLimitedAsyncReadWrite, FuturesRateLimiter, SharingRateLimiter};

struct RateLimitedStreamMuxer<SM> {
    rate_limiter: SharingRateLimiter,
    stream_muxer: SM,
}

impl<SM> RateLimitedStreamMuxer<SM> {
    pub fn new(stream_muxer: SM, rate_limiter: SharingRateLimiter) -> Self {
        Self {
            rate_limiter,
            stream_muxer,
        }
    }

    fn inner(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut SM>
    where
        SM: Unpin,
    {
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.stream_muxer)
    }
}

impl<SM> StreamMuxer for RateLimitedStreamMuxer<SM>
where
    SM: StreamMuxer + Unpin,
    SM::Substream: Unpin,
{
    type Substream = FuturesRateLimitedAsyncReadWrite<SM::Substream>;

    type Error = SM::Error;

    fn poll_inbound(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::Substream, Self::Error>> {
        let rate_limiter = self.rate_limiter.clone();
        self.inner().poll_inbound(cx).map(|result| {
            result.map(|substream| {
                FuturesRateLimitedAsyncReadWrite::new(
                    substream,
                    FuturesRateLimiter::new(rate_limiter),
                )
            })
        })
    }

    fn poll_outbound(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::Substream, Self::Error>> {
        let rate_limiter = self.rate_limiter.clone();
        self.inner().poll_outbound(cx).map(|result| {
            result.map(|substream| {
                FuturesRateLimitedAsyncReadWrite::new(
                    substream,
                    FuturesRateLimiter::new(rate_limiter),
                )
            })
        })
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner().poll_close(cx)
    }

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<libp2p::core::muxing::StreamMuxerEvent, Self::Error>> {
        self.inner().poll(cx)
    }
}

pub fn build_transport(
    rate_limiter: SharingRateLimiter,
    config: sc_network::transport::NetworkConfig,
) -> impl Transport<
    Output = (
        PeerId,
        impl StreamMuxer<Substream = impl Send, Error = impl Send> + Send,
    ),
    Dial = impl Send,
    ListenerUpgrade = impl Send,
    Error = impl Send,
> + Send {
    sc_network::transport::build_transport(
        config.keypair,
        config.memory_only,
        config.muxer_window_size,
        config.muxer_maximum_buffer_size,
    )
    .map(move |(peer_id, stream_muxer), _| {
        (
            peer_id,
            RateLimitedStreamMuxer::new(stream_muxer, rate_limiter),
        )
    })
}
