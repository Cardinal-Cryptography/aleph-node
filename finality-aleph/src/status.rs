use tokio::time::{self, Duration, Interval};

const STATUS_TICKER_DELAY: Duration = Duration::from_secs(10);

pub fn status_ticker() -> Interval {
    time::interval(STATUS_TICKER_DELAY)
}
