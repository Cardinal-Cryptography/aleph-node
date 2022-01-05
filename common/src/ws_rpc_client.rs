use log::info;
use serde_json::Value;
use sp_core::H256 as Hash;
use std::{
    sync::{
        mpsc::{channel, Sender as ThreadOut},
        Arc, Mutex,
    },
    thread,
    thread::JoinHandle,
};
use substrate_api_client::{
    rpc::{
        json_req,
        ws_client::{
            on_extrinsic_msg_submit_only, on_extrinsic_msg_until_broadcast,
            on_extrinsic_msg_until_finalized, on_extrinsic_msg_until_in_block,
            on_extrinsic_msg_until_ready, on_get_request_msg, OnMessageFn, RpcClient,
        },
    },
    ApiClientError, ApiResult, FromHexString, RpcClient as RpcClientTrait, XtStatus,
};
use ws::{connect, Handler, Message, Result as WsResult, Sender as WsSender};

pub struct WsRpcClient {
    mux: Mutex<()>,
    next_handler: Arc<Mutex<Option<RpcClient>>>,
    join_handle: Option<thread::JoinHandle<WsResult<()>>>,
    out: WsSender,
}

impl WsRpcClient {
    pub fn new(url: &str) -> WsRpcClient {
        let (sender, join_handle, rpc_client) = start_rpc_client_thread(url.to_string())
            .unwrap_or_else(|err| panic!("failed to spawn WebSocket's thread: {}", err));
        WsRpcClient {
            next_handler: rpc_client,
            join_handle: Some(join_handle),
            out: sender,
            mux: Mutex::new(()),
        }
    }
}

impl Drop for WsRpcClient {
    fn drop(&mut self) {
        self.close();
    }
}

impl RpcClientTrait for WsRpcClient {
    fn get_request(&self, jsonreq: Value) -> ApiResult<String> {
        let _mux = self.mux.lock();

        let (result_in, result_out) = channel();
        self.get(jsonreq.to_string(), result_in)?;

        let str = result_out.recv()?;

        // reset the RpcClient handler used by the WebSocket's thread
        *self
            .next_handler
            .lock()
            .expect("unable to acquire a lock on RpcClient") = None;

        Ok(str)
    }

    fn send_extrinsic(
        &self,
        xthex_prefixed: String,
        exit_on: XtStatus,
    ) -> ApiResult<Option<sp_core::H256>> {
        let _mux = self.mux.lock();

        // Todo: Make all variants return a H256: #175.

        let jsonreq = match exit_on {
            XtStatus::SubmitOnly => json_req::author_submit_extrinsic(&xthex_prefixed).to_string(),
            _ => json_req::author_submit_and_watch_extrinsic(&xthex_prefixed).to_string(),
        };

        let (result_in, result_out) = channel();
        let result = match exit_on {
            XtStatus::Finalized => {
                self.send_extrinsic_and_wait_until_finalized(jsonreq, result_in)?;
                let res = result_out.recv()?;
                info!("finalized: {}", res);
                Ok(Some(Hash::from_hex(res)?))
            }
            XtStatus::InBlock => {
                self.send_extrinsic_and_wait_until_in_block(jsonreq, result_in)?;
                let res = result_out.recv()?;
                info!("inBlock: {}", res);
                Ok(Some(Hash::from_hex(res)?))
            }
            XtStatus::Broadcast => {
                self.send_extrinsic_and_wait_until_broadcast(jsonreq, result_in)?;
                let res = result_out.recv()?;
                info!("broadcast: {}", res);
                Ok(None)
            }
            XtStatus::Ready => {
                self.send_extrinsic_until_ready(jsonreq, result_in)?;
                let res = result_out.recv()?;
                info!("ready: {}", res);
                Ok(None)
            }
            XtStatus::SubmitOnly => {
                self.send_extrinsic(jsonreq, result_in)?;
                let res = result_out.recv()?;
                info!("submitted xt: {}", res);
                Ok(None)
            }
            _ => Err(ApiClientError::UnsupportedXtStatus(exit_on)),
        };

        // reset the RpcClient handler used by the WebSocket's thread
        *self
            .next_handler
            .lock()
            .expect("unable to acquire a lock on RpcClient") = None;
        result
    }
}

impl WsRpcClient {
    fn get(&self, json_req: String, result_in: ThreadOut<String>) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_get_request_msg)
    }

    fn send_extrinsic(&self, json_req: String, result_in: ThreadOut<String>) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_extrinsic_msg_submit_only)
    }

    fn send_extrinsic_until_ready(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_extrinsic_msg_until_ready)
    }

    fn send_extrinsic_and_wait_until_broadcast(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_extrinsic_msg_until_broadcast)
    }

    fn send_extrinsic_and_wait_until_in_block(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_extrinsic_msg_until_in_block)
    }

    fn send_extrinsic_and_wait_until_finalized(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> WsResult<()> {
        self.send_rpc_request(json_req, result_in, on_extrinsic_msg_until_finalized)
    }

    fn send_rpc_request(
        &self,
        jsonreq: String,
        result_in: ThreadOut<String>,
        on_message_fn: OnMessageFn,
    ) -> WsResult<()> {
        // 1 used by `on_open` of RpcClient + 1 for `close`
        const MAGIC_SEND_CONST: usize = 2;
        let (ws_tx, _ws_rx) = mio::channel::sync_channel(MAGIC_SEND_CONST);
        let ws_sender = ws::Sender::new(0.into(), ws_tx, 0);

        let rpc_client = RpcClient {
            out: ws_sender,
            request: jsonreq.clone(),
            result: result_in,
            on_message_fn,
        };
        // force lock to be released before we send a message on ws::Sender, otherwise we might get a deadlock
        {
            let mut next_handler = self
                .next_handler
                .lock()
                .expect("unable to acquire a lock on RpcClient");
            *next_handler = Some(rpc_client);
        }
        self.out.send(jsonreq)
    }

    pub fn close(&mut self) {
        self.out
            .shutdown()
            .expect("unable to send close on the WebSocket");
        self.join_handle
            .take()
            .map(|handle| handle.join().expect("unable to join WebSocket's thread"));
    }
}

fn start_rpc_client_thread(
    url: String,
) -> Result<
    (
        WsSender,
        JoinHandle<WsResult<()>>,
        Arc<Mutex<Option<RpcClient>>>,
    ),
    String,
> {
    let (tx, rx) = std::sync::mpsc::sync_channel(0);
    let rpc_client = Arc::new(Mutex::new(None));
    let connect_rpc_client = Arc::clone(&rpc_client);
    let join = thread::Builder::new()
        .name("client".to_owned())
        .spawn(|| -> WsResult<()> {
            connect(url, move |out| {
                tx.send(out).expect("main thread was already stopped");
                WsHandler {
                    next_handler: connect_rpc_client.clone(),
                }
            })
        })
        .map_err(|_| "unable to spawn WebSocket's thread")?;
    let out = rx.recv().map_err(|_| "WebSocket's unexpectedly died")?;
    Ok((out, join, rpc_client))
}

struct WsHandler {
    next_handler: Arc<Mutex<Option<RpcClient>>>,
}

impl Handler for WsHandler {
    fn on_message(&mut self, msg: Message) -> WsResult<()> {
        if let Some(handler) = self
            .next_handler
            .lock()
            .expect("main thread probably died")
            .as_mut()
        {
            handler.on_message(msg)
        } else {
            Ok(())
        }
    }
}
