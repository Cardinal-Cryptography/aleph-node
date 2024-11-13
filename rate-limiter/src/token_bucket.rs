use crate::{NonZeroRatePerSecond, LOG_TARGET, MIN};
use futures::{future::pending, Future, FutureExt};
use log::trace;
use std::{
    cmp::min,
    num::NonZeroU64,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::time::sleep;

pub trait TimeProvider {
    fn now(&self) -> Instant;
}

#[derive(Clone, Default)]
pub struct DefaultTimeProvider;

impl TimeProvider for DefaultTimeProvider {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub trait SleepUntil {
    fn sleep_until(&mut self, instant: Instant) -> impl Future<Output = ()> + Send;
}

#[derive(Clone, Default)]
pub struct TokioSleepUntil;

impl SleepUntil for TokioSleepUntil {
    async fn sleep_until(&mut self, instant: Instant) {
        tokio::time::sleep_until(instant.into()).await;
    }
}

/// Implementation of the `Token Bucket` algorithm for the purpose of rate-limiting access to some abstract resource, e.g. an incoming network traffic.
#[derive(Clone)]
struct TokenBucket<T = DefaultTimeProvider> {
    last_update: Instant,
    rate_per_second: NonZeroU64,
    requested: u64,
    time_provider: T,
}

impl<T> std::fmt::Debug for TokenBucket<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenBucket")
            .field("last_update", &self.last_update)
            .field("rate_per_second", &self.rate_per_second)
            .field("requested", &self.requested)
            .finish()
    }
}

impl TokenBucket {
    /// Constructs a instance of [`TokenBucket`] with given target rate-per-second.
    pub fn new(rate_per_second: NonZeroRatePerSecond) -> Self {
        let time_provider = DefaultTimeProvider;
        let now = time_provider.now();
        Self {
            time_provider,
            last_update: now,
            rate_per_second: rate_per_second.into(),
            requested: NonZeroU64::from(rate_per_second).into(),
        }
    }
}

impl<TP> TokenBucket<TP>
where
    TP: TimeProvider,
{
    fn max_possible_available_tokens(&self) -> u64 {
        self.rate_per_second.into()
    }

    fn available(&self) -> Option<u64> {
        (self.requested <= self.max_possible_available_tokens())
            .then(|| self.max_possible_available_tokens() - self.requested)
    }

    fn account_requested_tokens(&mut self, requested: u64) {
        self.requested = self.requested.saturating_add(requested);
    }

    fn calculate_delay(&self) -> Option<Instant> {
        if self.available().is_some() {
            return None;
        }

        let scheduled_for_later = self.requested - self.max_possible_available_tokens();
        let delay_micros = scheduled_for_later
            .saturating_mul(1_000_000)
            .saturating_div(self.rate_per_second.into());

        Some(self.last_update + Duration::from_micros(delay_micros))
    }

    fn update_tokens(&mut self) {
        let now = self.time_provider.now();
        assert!(
            now >= self.last_update,
            "Provided value for `now` should be at least equal to `self.last_update`: now = {:#?} self.last_update = {:#?}.",
            now,
            self.last_update
        );

        let time_since_last_update = now.duration_since(self.last_update);
        self.last_update = now;
        let new_units = time_since_last_update
            .as_micros()
            .saturating_mul(u64::from(self.rate_per_second).into())
            .saturating_div(1_000_000)
            .try_into()
            .unwrap_or(u64::MAX);
        self.requested = self.requested.saturating_sub(new_units);
    }

    /// Get current rate in bits per second.
    pub fn rate(&self) -> NonZeroRatePerSecond {
        self.rate_per_second.into()
    }

    /// Set a rate in bits per second.
    pub fn set_rate(&mut self, rate_per_second: NonZeroRatePerSecond) {
        self.update_tokens();
        let available = self.available();
        let previous_rate_per_second = self.rate_per_second.get();
        self.rate_per_second = rate_per_second.into();
        if available.is_some() {
            let max_for_available = self.max_possible_available_tokens();
            let available_after_rate_update = min(available.unwrap_or(0), max_for_available);
            self.requested = self.rate_per_second.get() - available_after_rate_update;
        } else {
            self.requested = self.requested - previous_rate_per_second + self.rate_per_second.get();
        }
    }

    /// Calculates amount of time by which we should delay next call to some governed resource in order to satisfy
    /// specified rate limit.
    pub fn rate_limit(&mut self, requested: u64) -> Option<Instant> {
        trace!(
            target: LOG_TARGET,
            "TokenBucket called for {requested} of requested bytes. Internal state: {self:?}.",
        );
        let now_available = self.available().unwrap_or(0);
        if now_available < requested {
            self.update_tokens()
        }
        self.account_requested_tokens(requested);
        let delay = self.calculate_delay();
        trace!(
            target: LOG_TARGET,
            "TokenBucket calculated delay after receiving a request of {requested}: {delay:?}.",
        );
        delay
    }
}

/// Implementation of the bandwidth sharing strategy that attempts to assign equal portion of the total bandwidth to all active
/// consumers of the bandwidth.
pub struct SharedBandwidthManager {
    max_rate: NonZeroRatePerSecond,
    peers_count: Arc<AtomicU64>,
    already_requested: Option<NonZeroRatePerSecond>,
}

impl Clone for SharedBandwidthManager {
    fn clone(&self) -> Self {
        Self {
            max_rate: self.max_rate,
            peers_count: self.peers_count.clone(),
            already_requested: None,
        }
    }
}

impl SharedBandwidthManager {
    fn calculate_bandwidth_without_children_increament(
        &mut self,
        active_children: Option<u64>,
    ) -> NonZeroRatePerSecond {
        let active_children =
            active_children.unwrap_or_else(|| self.peers_count.load(Ordering::Relaxed));
        let rate = u64::from(self.max_rate) / active_children;
        NonZeroU64::try_from(rate)
            .map(NonZeroRatePerSecond::from)
            .unwrap_or(MIN)
    }
}

impl SharedBandwidthManager {
    pub fn new(max_rate: NonZeroRatePerSecond) -> Self {
        Self {
            max_rate,
            peers_count: Arc::new(AtomicU64::new(0)),
            already_requested: None,
        }
    }
}

impl SharedBandwidthManager {
    pub fn request_bandwidth(&mut self) -> NonZeroRatePerSecond {
        let active_children = (self.already_requested.is_none())
            .then(|| 1 + self.peers_count.fetch_add(1, Ordering::Relaxed));
        let rate = self.calculate_bandwidth_without_children_increament(active_children);
        self.already_requested = Some(rate);
        rate
    }

    pub fn notify_idle(&mut self) {
        if self.already_requested.take().is_some() {
            self.peers_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub async fn bandwidth_changed(&mut self) -> NonZeroRatePerSecond {
        let Some(previous_rate) = self.already_requested else {
            return pending().await;
        };
        let sleep_amount = Duration::from_millis(250);
        let mut rate = self.calculate_bandwidth_without_children_increament(None);
        while rate == previous_rate {
            sleep(sleep_amount).await;
            rate = self.calculate_bandwidth_without_children_increament(None);
        }
        self.already_requested = Some(rate);
        rate
    }
}

/// Wrapper around the [TokenBucket] that allows conveniently manage its internal token-rate and allows to idle/sleep in order
/// to fulfill its rate-limit.
#[derive(Clone)]
struct AsyncTokenBucket<TP = DefaultTimeProvider, SU = TokioSleepUntil> {
    token_bucket: TokenBucket<TP>,
    next_deadline: Option<Instant>,
    sleep_until: SU,
}

impl<TP, SU> AsyncTokenBucket<TP, SU> {
    pub fn new(token_bucket: TokenBucket<TP>, sleep_until: SU) -> Self {
        Self {
            token_bucket,
            next_deadline: None,
            sleep_until,
        }
    }
}

impl<TP, SU> AsyncTokenBucket<TP, SU>
where
    TP: TimeProvider,
{
    pub fn rate_limit(&mut self, requested: u64) {
        self.next_deadline = TokenBucket::rate_limit(&mut self.token_bucket, requested);
    }

    pub fn set_rate(&mut self, rate: NonZeroRatePerSecond) {
        if self.token_bucket.rate() != rate {
            self.token_bucket.set_rate(rate);
            self.next_deadline = self.token_bucket.rate_limit(0);
        }
    }

    pub async fn wait(&mut self)
    where
        TP: TimeProvider + Send,
        SU: SleepUntil + Send,
    {
        if let Some(deadline) = self.next_deadline {
            self.sleep_until.sleep_until(deadline).await;
            self.next_deadline = None;
        }
    }
}

/// Allows to share a given amount of bandwidth between multiple instances of [TokenBucket].
#[derive(Clone)]
pub struct SharedTokenBucket<TP = DefaultTimeProvider, SU = TokioSleepUntil> {
    shared_bandwidth: SharedBandwidthManager,
    rate_limiter: AsyncTokenBucket<TP, SU>,
    need_to_notify_parent: bool,
}

impl SharedTokenBucket {
    pub fn new(rate: NonZeroRatePerSecond) -> Self {
        let token_bucket = TokenBucket::new(rate);
        let sleep_until = TokioSleepUntil;
        let rate_limiter = AsyncTokenBucket::new(token_bucket, sleep_until);
        Self {
            shared_bandwidth: SharedBandwidthManager::new(rate),
            rate_limiter,
            need_to_notify_parent: false,
        }
    }
}

impl<TP, SU> SharedTokenBucket<TP, SU> {
    fn request_bandwidth(&mut self) -> NonZeroRatePerSecond {
        self.need_to_notify_parent = true;
        self.shared_bandwidth.request_bandwidth()
    }

    fn notify_idle(&mut self) {
        if self.need_to_notify_parent {
            self.shared_bandwidth.notify_idle();
            self.need_to_notify_parent = false;
        }
    }

    pub async fn rate_limit(mut self, requested: u64) -> Self
    where
        TP: TimeProvider + Send,
        SU: SleepUntil + Send,
    {
        let rate = self.request_bandwidth();
        self.rate_limiter.set_rate(rate);

        self.rate_limiter.rate_limit(requested);

        loop {
            futures::select! {
                _ = self.rate_limiter.wait().fuse() => {
                    self.notify_idle();
                    return self;
                },
                rate = self.shared_bandwidth.bandwidth_changed().fuse() => {
                    self.rate_limiter.set_rate(rate);
                },
            }
        }
    }
}

impl<TP, SU> Drop for SharedTokenBucket<TP, SU> {
    fn drop(&mut self) {
        self.notify_idle();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        time::{Duration, Instant},
    };

    use crate::token_bucket::{Deadline, TokenBucket};

    #[test]
    fn token_bucket_sanity_check() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert_eq!(rate_limiter.rate_limit(9), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert!(rate_limiter.rate_limit(12).is_some());

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(8), None);
    }

    #[test]
    fn no_slowdown_while_within_rate_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(1);
        assert_eq!(rate_limiter.rate_limit(9), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(2);
        assert_eq!(rate_limiter.rate_limit(5), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(1), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(9), None);
    }

    #[test]
    fn slowdown_when_limit_reached() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now;
        assert_eq!(rate_limiter.rate_limit(10), None);

        // we should wait some time after reaching the limit
        *time_to_return.borrow_mut() = now;
        assert!(rate_limiter.rate_limit(1).is_some());

        *time_to_return.borrow_mut() = now;
        assert_eq!(
            rate_limiter.rate_limit(19),
            Some(Deadline::Instant(now + Duration::from_secs(2))),
            "we should wait exactly 2 seconds"
        );
    }

    #[test]
    fn buildup_tokens_but_no_more_than_limit() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(2);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(10);
        assert_eq!(
            rate_limiter.rate_limit(40),
            Some(Deadline::Instant(
                now + Duration::from_secs(10) + Duration::from_secs(3)
            )),
        );

        *time_to_return.borrow_mut() = now + Duration::from_secs(11);
        assert_eq!(
            rate_limiter.rate_limit(40),
            Some(Deadline::Instant(
                now + Duration::from_secs(11) + Duration::from_secs(6)
            ))
        );
    }

    #[test]
    fn multiple_calls_buildup_wait_time() {
        let limit_per_second = 10;
        let now = Instant::now();
        let time_to_return = RefCell::new(now);
        let time_provider = || *time_to_return.borrow();
        let mut rate_limiter = TokenBucket::new_with_now(limit_per_second, now, time_provider);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(rate_limiter.rate_limit(10), None);

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(
            rate_limiter.rate_limit(10),
            Some(Deadline::Instant(
                now + Duration::from_secs(3) + Duration::from_secs(1)
            ))
        );

        *time_to_return.borrow_mut() = now + Duration::from_secs(3);
        assert_eq!(
            rate_limiter.rate_limit(50),
            Some(Deadline::Instant(
                now + Duration::from_secs(3) + Duration::from_secs(6)
            ))
        );
    }
}
