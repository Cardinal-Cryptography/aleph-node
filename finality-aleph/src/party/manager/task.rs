use std::{boxed::Box, pin::Pin};

use futures::channel::oneshot;
use log::warn;

use crate::Future;

/// A single handle that can be waited on, as returned by spawning an essential task.
pub type Handle = Pin<Box<(dyn Future<Output = sc_service::Result<(), ()>> + Send + 'static)>>;

/// A task that can be stopped or awaited until it stops itself.
pub struct Task {
    handle: Handle,
    exit: oneshot::Sender<()>,
    error_on_exit: Option<bool>,
}

impl Task {
    /// Create a new task.
    pub fn new(handle: Handle, exit: oneshot::Sender<()>) -> Self {
        Task {
            handle,
            exit,
            error_on_exit: None,
        }
    }

    /// Cleanly stop the task.
    pub async fn stop(self) -> Result<(), ()> {
        if let Some(res) = self.error_on_exit {
            return if res { Err(()) } else { Ok(()) };
        }
        if let Err(e) = self.exit.send(()) {
            warn!(target: "aleph-party", "Failed to send exit signal to authority: {:?}", e);
            return if let Some(true) = self.error_on_exit {
                Err(())
            } else {
                Ok(())
            };
        }
        self.handle.await
    }

    /// Await the task to stop by itself. Should usually just block forever, unless something went
    /// wrong. Can be called multiple times.
    pub async fn stopped(&mut self) -> Result<(), ()> {
        if let Some(res) = self.error_on_exit {
            return if res { Err(()) } else { Ok(()) };
        }
        let result = (&mut self.handle).await;
        self.error_on_exit = Some(result.is_err());
        result
    }
}
