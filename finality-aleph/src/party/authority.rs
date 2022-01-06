use crate::{
    party::{Handle, Task as PureTask},
    NodeIndex, SpawnHandle,
};
use futures::channel::oneshot;

pub struct Task {
    task: PureTask,
    node_id: NodeIndex,
}

impl Task {
    pub fn new(handle: Handle, node_id: NodeIndex, exit: oneshot::Sender<()>) -> Self {
        Task {
            task: PureTask::new(handle, exit),
            node_id,
        }
    }

    pub async fn stop(self) {
        self.task.stop().await
    }

    pub async fn stopped(&mut self) -> NodeIndex {
        self.task.stopped().await;
        self.node_id
    }
}

pub struct Subtasks {
    exit: oneshot::Receiver<()>,
    member: PureTask,
    aggregator: PureTask,
    forwarder: PureTask,
    refresher: PureTask,
    data_store: PureTask,
}

impl Subtasks {
    pub fn new(
        exit: oneshot::Receiver<()>,
        member: PureTask,
        aggregator: PureTask,
        forwarder: PureTask,
        refresher: PureTask,
        data_store: PureTask,
    ) -> Self {
        Subtasks {
            exit,
            member,
            aggregator,
            forwarder,
            refresher,
            data_store,
        }
    }

    async fn stop(self) {
        // both member and aggregator are implicitly using forwarder,
        // so we should force them to exit first to avoid any panics, i.e. `send on closed channel`
        self.member.stop().await;
        self.aggregator.stop().await;
        self.forwarder.stop().await;
        self.refresher.stop().await;
        self.data_store.stop().await;
    }

    pub async fn failed(mut self) -> bool {
        let result = tokio::select! {
            _ = &mut self.exit => false,
            _ = self.member.stopped() => true,
            _ = self.aggregator.stopped() => true,
            _ = self.forwarder.stopped() => true,
            _ = self.refresher.stopped() => true,
            _ = self.data_store.stopped() => true,
        };
        self.stop().await;
        result
    }
}

#[derive(Clone)]
pub struct SubtaskCommon {
    pub spawn_handle: SpawnHandle,
    pub session_id: u32,
}
