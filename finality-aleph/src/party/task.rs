use crate::Future;
use futures::channel::oneshot;
use log::warn;
use std::{boxed::Box, pin::Pin};

pub type Handle = Pin<Box<(dyn Future<Output = sc_service::Result<(), ()>> + Send + 'static)>>;

pub struct Task {
    handle: Handle,
    exit: oneshot::Sender<()>,
}

impl Task {
    pub fn new(handle: Handle, exit: oneshot::Sender<()>) -> Self {
        Task { handle, exit }
    }

    pub async fn stop(self) {
        if let Err(e) = self.exit.send(()) {
            warn!(target: "aleph-party", "Failed to send exit signal to authority: {:?}", e);
        }
        let _ = self.handle.await;
    }

    pub async fn stopped(&mut self) {
        let _ = (&mut self.handle).await;
    }
}
