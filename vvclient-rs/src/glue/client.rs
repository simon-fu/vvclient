
use anyhow::Result;

use const_format::formatcp;

use log::{debug, warn};

use parking_lot::Mutex;

use tokio::sync::oneshot;
use tracing::instrument;

use trace_error::anyhow::trace_result;

use core::fmt;

use std::{collections::{HashMap, VecDeque}, sync::Arc};

use crate::{client::{defines::JoinConfig, xfer::Xfer}, kit::astr::AStr, proto};

use crate::client::worker::{Delegate, Session, Worker};

use super::defines as records;

use super::error::Error;
type GlueResult<T, E = Error> = std::result::Result<T, E>;

use crate::glue::error::ForeignResult;
// use crate::glue::error::ForeignError;
// pub type ForeignResult<T, E = ForeignError> = std::result::Result<T, E>;


#[uniffi::export]
#[trace_result]
pub fn make_signal_client(url: String, config: records::SignalConfig, listener: Arc<dyn Listener>) -> GlueResult<SignalClient> {

    let delegate = DelegateImpl {
        listener, 
        shared: Default::default(),
        users: Default::default(),
        response_handlers: Default::default(),
        dlink_x_id: Default::default(),
        got_room_ready: false,
    };

    let shared = delegate.shared.clone();

    let mut config: JoinConfig = config.into();
    config.advance.batch = Some(true);

    let worker = Worker::try_new(url, None, config, delegate)?;

    Ok(SignalClient {
        worker,
        shared,
    })
}

#[derive(uniffi::Object)]
pub struct SignalClient {
    worker: Worker,
    shared: Arc<Shared<Arc<dyn Listener>>>,
}


#[uniffi::export]
impl SignalClient {

    // #[trace_result]
    // pub fn leave_room(&self, end: Option<bool>) {
    //     let r = self.commit_op(Op::Leave {end, tx: None});
    //     if let Err(e) = r {
    //         warn!("{}", trace_fmt!("close failed", e));
    //     }
    // }

    #[trace_result]
    pub async fn leave_room_and_wait(&self, end: Option<bool>, force: bool) {
        let (tx, rx) = oneshot::channel();
        if self.shared.is_closed() {
            debug!("leave_room_and_wait: already closed, skipping");
            return;
        }
        self.worker.set_shutdown_mode(force);
        // TODO(ios-compat): ensure leave wait unblocks even if on_closed arrives before Op::Leave is processed.
        self.shared.set_leave_tx(tx);
        debug!("leave_room_and_wait: commit_op start");
        let r = self.commit_op(Op::Leave {end, tx: None});
        let _ = self.worker.commit();
        match r {
            Ok(()) => {
                debug!("leave_room_and_wait: commit_op ok, awaiting on_closed");
                let r = rx.await;
                debug!("leave_room_and_wait: on_closed received, is_ok {}", r.is_ok());
            },
            Err(e) => {
                self.shared.clear_leave_tx();
                warn!("leave_room_and_wait: commit_op failed: {}", trace_fmt!("leave failed", e));
            }
        }
    }


    #[trace_result]
    pub fn conn_x(&self, req: records::ConnXCall, cb: Arc<dyn CbResolveConnX>) -> GlueResult<()> {
        self.simple_request(req, move |rsp| cb.resolve(rsp))?;
        Ok(())
    }

    /// return fake producer_id
    #[trace_result]
    pub fn pub_stream(&self, req: records::PubCall, cb: Arc<dyn CbResolvePub>) -> GlueResult<String> {
        let next_id = 0; // TODO: aaa
        let fake_producer_id = format!("{}_{}", req.stype.as_str(), next_id);
        self.simple_request(req, move |rsp| cb.resolve(rsp))?;
        Ok(fake_producer_id)
    }

    #[trace_result]
    pub fn upub_stream(&self, req: records::UPubCall, cb: Arc<dyn OnResolveUPub>) -> GlueResult<()> {
        self.simple_request(req, move |rsp| cb.resolve(rsp))?;
        Ok(())
    }
    

    #[trace_result]
    pub fn mute_stream(&self, req: records::MuteCall, cb: Arc<dyn CbResolveMute>) -> GlueResult<()> {
        self.simple_request(req, move |rsp| cb.resolve(rsp))?;
        Ok(())
    }

    #[trace_result]
    pub fn sub_stream(&self, req: records::SubCall, cb: Arc<dyn CbResolveSub>) -> GlueResult<()> {

        self.commit_op(Op::Request(Box::new(move |delegate, session| {
            let user = delegate.users.get(req.user_id.as_str()).with_context(||format!("Not found user"))?;

            let x_id = delegate.dlink_x_id.as_ref().with_context(||"no dlink_x_id")?.clone();

            let (stream_id, stream) = user.state.streams.iter()
                .find(|x| x.1.stype == req.stype.value())
                .with_context(||format!("can't find stream for sub {req:?}"))?; // TODO: 可恢复错误

            let producer_id = stream.producer_id.clone();
            let stream_id = stream_id.clone();

            let body = proto::SubscribeRequestSer {
                room_id: "".into(), 
                stream_id: &stream_id, 
                producer_id: &producer_id, 
                xid: &x_id, 
                preferred_layers: None, // TODO: 从 SubCall 获取 preferred_layers
            }.into_body();

            let sn = session.xfer_mut().add_qos1_json(proto::PacketType::Request, &body, None)?;
            delegate.response_handlers.insert(sn, Box::new(move |_delegate, response| {
                let ret = records::SubReturn::try_new(x_id, producer_id, stream_id, response)?;
                cb.resolve(ret)?;
                Ok(())
            }));
            Ok(())
        })))?;

        Ok(())

    }

    #[trace_result]
    pub fn unsub_stream(&self, req: records::UnsubCall, cb: Arc<dyn CbResolveUSub>) -> GlueResult<()> {

        self.commit_op(Op::Request(Box::new(move |delegate, session| {

            let body = proto::UnsubscribeRequestSer {
                room_id: "".into(), 
                consumer_id: &req.consumer_id,
            }.into_body();

            let sn = session.xfer_mut().add_qos1_json(proto::PacketType::Request, &body, None)?;
            delegate.response_handlers.insert(sn, Box::new(move |_delegate, response| {
                let ret = records::UnsubReturn::try_new(response)?;
                cb.resolve(ret)?;
                Ok(())
            }));
            Ok(())
        })))?;

        Ok(())

    }
}


#[uniffi::export(with_foreign)]
pub trait CbResolveConnX : Send + Sync {
    fn resolve(&self, response: records::ConnXReturn) -> ForeignResult<()>;
}

#[uniffi::export(with_foreign)]
pub trait CbResolvePub : Send + Sync {
    fn resolve(&self, response: records::PubReturn) -> ForeignResult<()>;
}

#[uniffi::export(with_foreign)]
pub trait OnResolveUPub : Send + Sync {
    fn resolve(&self, response: records::UPubReturn) -> ForeignResult<()>;
}

#[uniffi::export(with_foreign)]
pub trait CbResolveMute : Send + Sync {
    fn resolve(&self, response: records::MuteReturn) -> ForeignResult<()>;
}

#[uniffi::export(with_foreign)]
pub trait CbResolveSub : Send + Sync {
    fn resolve(&self, response: records::SubReturn) -> ForeignResult<()>;
}

#[uniffi::export(with_foreign)]
pub trait CbResolveUSub : Send + Sync {
    fn resolve(&self, response: records::UnsubReturn) -> ForeignResult<()>;
}

impl SignalClient {
    pub async fn into_finish(self) {
        self.worker.into_finish().await
    }

    #[trace_result]
    fn simple_request<REQ, RSP, F>(&self, req: REQ, func: F) -> GlueResult<()> 
    where 
        REQ: records::ToBody + Send + Sync + 'static,
        RSP: TryFrom<proto::response::ResponseType, Error = anyhow::Error> + Send + Sync + 'static,
        F: FnOnce(RSP) -> ForeignResult<()> + Send + Sync + 'static,
    {

        self.commit_op(Op::Request(Box::new(move |delegate, session| {

            let body = req.to_body()?;

            let sn = session.xfer_mut().add_qos1_json(proto::PacketType::Request, &body, None)?;

            delegate.response_handlers.insert(sn, Box::new(move |_delegate, response| {
                func(response.try_into()?)?;
                Ok(())
            }));

            Ok(())

        })))?;

        Ok(())
    }

    #[trace_result]
    fn commit_op(&self, op: Op<Arc<dyn Listener>>) -> GlueResult<()> {
        {
            self.shared.inbox.lock().push_back(op);
        }
        self.worker.commit()?;
        Ok(())
    }
}



struct DelegateImpl<L> {
    listener: L,
    shared: Arc<Shared<L>>,
    response_handlers: HashMap<i64, ResponseResolver<L>>,
    users: HashMap<AStr, UserCell>,
    dlink_x_id: Option<String>,
    got_room_ready: bool,
}

impl<L: Listener> DelegateImpl<L> {
    // fn is_ready(&self) -> bool {
    //     self.got_room_ready && self.dlink_x_id.is_some()
    // }

    // #[trace_result]
    // fn check_fire_room_ready(&self) -> Result<()> {
    //     if !self.is_ready() {
    //         return Ok(())
    //     }

    //     self.listener.on_room_ready()?;

    //     Ok(())
    // }

    #[trace_result]
    async fn process_op(&mut self, session: &mut Session, msg: Op<L>) -> Result<()> {
        match msg {
            Op::Leave{end, tx}=>{
                debug!("process_op: Op::Leave start end={:?} tx_present={}", end, tx.is_some());
                // session.set_close(status.into());
                self.req_leave(session.xfer_mut(), end, tx)?;
                debug!("process_op: Op::Leave request queued");
            },
            Op::Request(func) => {
                func(self, session)?;
            }
        }
        Ok(())
    }

    #[trace_result]
    fn req_create_x(&mut self, xfer: &mut Xfer, dir: i32) -> Result<()> {

        let body = proto::CreateWebrtcTransportRequestSer {
            room_id: "".into(),
            dir,
            kind: 0,
            dtls: Option::<()>::None,
        }.into_body();

        self.send_request(xfer, &body, Box::new(move |delegate, response| {
            let rsp = records::OnCreateXResponse::try_from(response)?;

            if dir == proto::Direction::Outbound.value() {
                delegate.dlink_x_id = Some(rsp.xid.clone());
                // delegate.check_fire_room_ready()?;
            }
            
            delegate.listener.on_created_x(OnCreatedXArgs {
                dir,
                response: rsp,
            })?;

            Ok(())
        }))?;

        Ok(())
    }

    #[trace_result]
    fn req_batch_end(&mut self, xfer: &mut Xfer) -> Result<()> {

        let body = proto::BatchEndRequestSer {
            
        }.into_body();

        self.send_request(xfer, &body, Box::new(move |_delegate, _response| {

            Ok(())
        }))?;

        Ok(())
    }

    #[trace_result]
    fn req_leave(&mut self, xfer: &mut Xfer, end: Option<bool>, tx: Option<oneshot::Sender<()>>) -> Result<()> {

        let body = proto::CloseSessionRequestSer {
            room_id: "".into(),
            end,
        }.into_body();

        debug!("req_leave: sending CloseSession");
        self.send_request(xfer, &body, Box::new(move |_delegate, response| {
            match response {
                proto::response::ResponseType::Close(_rsp) => {
                    debug!("req_leave: CloseSession response received");
                },
                _ => {
                    return Err(anyhow::anyhow!("invalid response type for CloseSession"));
                }
            }  

            Ok(())
        }))?;

        debug!("req_leave: request queued, storing leave_tx {}", tx.is_some());

        Ok(())
    }

    #[trace_result]
    fn send_request<B>(&mut self, xfer: &mut Xfer, body: &B, resolver: ResponseResolver<L>) -> Result<()>
    where 
        B: serde::Serialize + fmt::Display,
    {
        let sn = xfer.add_qos1_json(proto::PacketType::Request, body, None)?;

        self.response_handlers.insert(sn, resolver);

        Ok(())
    }

    #[trace_result]
    #[instrument(skip(self, _session, push))]
    async fn handle_server_push(&mut self, _session: &mut Session, push: proto::ServerPush) -> Result<()> {
        match push.typ {
            proto::ServerPushType::Closed(notice) => {
                return Err(notice.status.unwrap_or_default())
            },
            proto::ServerPushType::Chat(notice) => {
                const CHAT_DIV: char = '.';
                const USER_PREFIX: &'static str = formatcp!("u{CHAT_DIV}"); 
                const ROOM_PREFIX: &'static str = formatcp!("r{CHAT_DIV}"); 

                let Some(from_user_id) = notice.from.strip_prefix(USER_PREFIX) else {
                    warn!("unknown chat.from [{}]", notice.from);
                    return Ok(());
                };

                if let Some(room_id) = notice.to.strip_prefix(ROOM_PREFIX) {

                    self.listener.on_room_chat(OnRoomChatArgs {
                        chat: records::ChatArgs {
                            from: from_user_id.into(), 
                            to: room_id.into(), 
                            body: notice.body,
                        }
                    })?;

                } else if let Some(to_user_id) = notice.to.strip_prefix(USER_PREFIX) {
                    self.listener.on_user_chat(OnUserChatArgs {
                        chat: records::ChatArgs {
                        from: from_user_id.into(), 
                        to: to_user_id.into(),
                        body: notice.body,
                        }
                    })?;

                } else {
                    warn!("unknown chat.to [{}]", notice.to);
                    return Ok(());
                }
            },
            proto::ServerPushType::UInit(id) => {
                let user_id = id.id;
                if let Some(cell) = self.users.remove(user_id.as_str()) {
                    debug!("got user init but has user [{cell:?}]");
                    self.listener.on_user_leaved(OnUserLeavedArgs {
                        user_id,
                    })?;
                }
            },
            proto::ServerPushType::UReady(id) => {
                // let is_ready = self.is_ready();

                let user_id: String = id.id;

                let Some(user) = self.users.get_mut(user_id.as_str()) else {
                    warn!("got user ready but NOT found user [{user_id}]");
                    return Ok(())
                };

                match &user.stage {
                    UserStage::Init => {},
                    UserStage::Ready => {
                        warn!("already ready but got user ready again, user_id [{user_id}]");
                    },
                }

                user.stage = UserStage::Ready;

                if self.got_room_ready {
                    fire_user_joined(&self.listener, &user_id, user)?;
                }

                // self.handle_extra_user(session, &user_id).await?;
            },
            proto::ServerPushType::UFull(new_state) => {
                let r = self.users.remove_entry(new_state.id.as_str());

                let (user_id, cell) = match r {
                    Some((user_id, mut cell)) => {
                        match &cell.stage {
                            UserStage::Init => {
                                if !new_state.online {
                                    return Ok(())
                                }
                            },
                            UserStage::Ready => {
                                if !new_state.online {
                                    // debug!("leaved user [{user_id}]");
                                    self.listener.on_user_leaved(OnUserLeavedArgs {
                                        user_id: user_id.to_string(),
                                    })?;
                                    return Ok(())
                                }

                                if new_state.inst_id != cell.state.inst_id {
                                    // 不应该走到这里
                                    warn!("got new user instance, old [{}], new [{}]", cell.state.inst_id, new_state.inst_id);
                                    self.listener.on_user_leaved(OnUserLeavedArgs {
                                        user_id: user_id.as_str().to_owned(),
                                    })?;
                                    cell.stage = UserStage::Init;
                                } else {
                                    cell.delta(&new_state, &mut self.listener)?;    
                                }

                            },
                        }
                        cell.state = new_state;
                        (user_id, cell)
                    },
                    None => {
                        let user_id: AStr = new_state.id.clone().into();
                        (
                            user_id.clone(),
                            UserCell {
                                user_id,
                                state: new_state, 
                                tree: Default::default(), 
                                stage: UserStage::Init,
                            }
                        )
                    },
                };

                self.users.insert(user_id, cell);
            },
            proto::ServerPushType::UTree(mut tree_state) => {
                let user_id = tree_state.id.take().with_context(||"no user_id in user tree")?;
                let user_id: AStr = user_id.into();

                let Some(cell) = self.users.get_mut(&user_id) else {
                    warn!("got user tree but has no user [{user_id}]");
                    return Ok(())
                };

                match &cell.stage {
                    UserStage::Init => {
                        cell.tree.push(tree_state.into());
                    },
                    UserStage::Ready => {
                        self.listener.on_update_user_tree(OnUpdateUserTreeArgs {
                            user_id: user_id.to_string(), 
                            node: tree_state.into(),
                        })?;
                    },
                }
            },
            proto::ServerPushType::RTree(tree_state) => {
                self.listener.on_update_room_tree(OnUpdateRoomTreeArgs {
                    node: tree_state.into(),
                })?;
            },
            proto::ServerPushType::RReady(_id) => {
                self.listener.on_room_ready()?;
                self.got_room_ready = true;
                // self.check_fire_room_ready()?;

                for (user_id, user) in self.users.iter_mut() {
                    fire_user_joined(&self.listener, &user_id, user)?;
                }
                

            },
        }
        Ok(())
    }
}

fn fire_user_joined<L: Listener>(listener: &L, user_id: &str, user: &mut UserCell) -> Result<()> {

    let tree = core::mem::replace(&mut user.tree, Vec::new());

    let brief = user.brief();
    listener.on_user_joined(OnUserJoinedArgs {
        user_id: user_id.to_owned(), 
        tree,
    })?;

    if let Some(muted) = brief.camera_muted {
        listener.on_add_stream(OnAddStreamArgs {
            user_id: user_id.to_owned(), 
            muted,
            stype: records::StreamType::Camera, 
        })?;
    }
    
    if let Some(muted) = brief.mic_muted {
        listener.on_add_stream(OnAddStreamArgs {
            user_id: user_id.to_owned(), 
            muted,
            stype: records::StreamType::Mic, 
        })?;
    }

    if let Some(muted) = brief.screen_muted {
        listener.on_add_stream(OnAddStreamArgs {
            user_id: user_id.to_owned(), 
            muted,
            stype: records::StreamType::Screen, 
        })?;
    }
    Ok(())
}

#[allow(unused)]
impl<L: Listener> Delegate for DelegateImpl<L> {

    #[instrument(skip_all)]
    #[trace_result]
    async fn on_opening(&mut self, xfer: &mut Xfer) -> Result<()> {
        self.req_create_x(xfer, proto::Direction::Inbound.value())?;
        self.req_create_x(xfer, proto::Direction::Outbound.value())?;
        self.req_batch_end(xfer)?;
        Ok(())
    }

    #[instrument(skip_all)]
    #[trace_result]
    async fn on_opened(&mut self, session: &mut Session) -> Result<()> {
        self.listener.on_opened(OnOpenedArgs {
            session_id: session.session_id().into(),
            // create_x_responses: create_x_responses.map(|x|{
            //     x.into_iter()
            //     .map(|x|records::OnCreateXResponse::try_from(x))
            //     .collect()
            // }).transpose()?,
        })?;
        Ok(())
    }

    #[instrument(skip_all)]
    async fn on_closed(&mut self, status: proto::Status) -> Result<()> {
        self.shared.set_closed();
        debug!("recv on_closed, status [{:?}], leave_tx {}", status, self.shared.has_leave_tx());
        if let Some(tx) = self.shared.take_leave_tx() {
            let r = tx.send(());
            debug!("sent leave_tx, is_ok {}", r.is_ok());
        }

        self.listener.on_closed(OnClosedArgs {
            status: status.into(),
        })?;
        Ok(())
    }

    #[instrument(skip_all)]
    #[trace_result]
    async fn on_process(&mut self, session: &mut Session) -> Result<()> {
        while let Some(msg) = self.shared.pop_inbox() {
            self.process_op(session, msg).await?;
        }

        Ok(())
    }

    #[instrument(skip_all)]
    #[trace_result]
    async fn on_response(&mut self, session: &mut Session, ack: i64, mut response: proto::ServerResponse) -> Result<()> {
        let resolver = self.response_handlers.remove(&ack)
            .with_context(||format!("Not found response handler for ack [{ack}]"))?;

        let status = response.status.take().unwrap_or_default();
        if status.code != 0 {
            warn!("response failed, ack [{ack}], {status:?}");
            return Ok(())
        }

        resolver(self, response.typ.with_context(||"no typ field in response")?)?;

        Ok(())
    }

    #[instrument(skip_all)]
    #[trace_result]
    async fn on_push(&mut self, session: &mut Session, push: proto::ServerPush) -> Result<()>  {
        self.handle_server_push(session, push).await
    }

}


struct Shared<L> {
    inbox: Mutex<VecDeque<Op<L>>>,
    leave_tx: Mutex<Option<oneshot::Sender<()>>>,
    closed: Mutex<bool>,
}

impl<L> Default for Shared<L> {
    fn default() -> Self {
        Self { inbox: Default::default(), leave_tx: Default::default(), closed: Mutex::new(false) }
    }
}

impl<L> Shared<L> {
    fn pop_inbox(&self) -> Option<Op<L>> {
        self.inbox.lock().pop_front()
    }

    fn set_leave_tx(&self, tx: oneshot::Sender<()>) {
        *self.leave_tx.lock() = Some(tx);
    }

    fn take_leave_tx(&self) -> Option<oneshot::Sender<()>> {
        self.leave_tx.lock().take()
    }

    fn clear_leave_tx(&self) {
        self.leave_tx.lock().take();
    }

    fn has_leave_tx(&self) -> bool {
        self.leave_tx.lock().is_some()
    }

    fn set_closed(&self) {
        *self.closed.lock() = true;
    }

    fn is_closed(&self) -> bool {
        *self.closed.lock()
    }
}


enum Op<L> {
    Leave { end: Option<bool>, tx: Option<oneshot::Sender<()>> },
    Request(RequestBuilder<L>),
}

type RequestBuilder<L> = Box<dyn FnOnce(&mut DelegateImpl<L>, &mut Session) -> Result<()> + Send + Sync>;
type ResponseResolver<L> = Box<dyn FnOnce(&mut DelegateImpl<L>, proto::response::ResponseType) -> Result<()> + Send + Sync>;


#[derive(Debug)]
struct UserCell {
    user_id: AStr, 
    state: proto::UserState,
    tree: Vec<records::TreeNode>,
    stage: UserStage,
}

impl UserCell {
    #[trace_result]
    pub fn delta<L>(&self, new_state: &proto::UserState, listener: &mut L) -> Result<()>
    where 
        L: Listener,
    {
        let old_state = &self.state;

        for (stream_id, old_stream) in old_state.streams.iter() {
            match new_state.streams.get(stream_id) {
                Some(new_stream) => {
                    if old_stream.producer_id != new_stream.producer_id {
                        listener.on_remove_stream(OnRemoveStreamArgs {
                            user_id: self.user_id.to_string(), 
                            stype: records::StreamType::from_value(old_stream.stype)?,
                        })?;
                        listener.on_add_stream(OnAddStreamArgs {
                            user_id: self.user_id.to_string(), 
                            muted: new_stream.muted,
                            stype: records::StreamType::from_value(new_stream.stype)?, 
                        })?;
                    } else {
                        if old_stream.muted != new_stream.muted {
                            listener.on_update_stream(OnUpdateStreamArgs {
                                user_id: self.user_id.to_string(), 
                                muted: new_stream.muted,
                                stype: records::StreamType::from_value(old_stream.stype)?,
                            })?;
                        }
                    }
                },
                None => {
                    listener.on_remove_stream(OnRemoveStreamArgs {
                        user_id: self.user_id.to_string(), 
                        stype: records::StreamType::from_value(old_stream.stype)?,
                    })?;
                },
            }
        }

        for (stream_id, new_stream) in new_state.streams.iter() {

            match old_state.streams.get(stream_id) {
                Some(_old_stream) => {},
                None => {
                    listener.on_add_stream(OnAddStreamArgs {
                        user_id: self.user_id.to_string(), 
                        muted: new_stream.muted,
                        stype: records::StreamType::from_value(new_stream.stype)?, 
                    })?;
                },
            }
        }

        Ok(())
    }

    pub fn brief(&self) -> UserBrief {
        UserBrief {
            camera_muted: self.stream_muted(proto::StreamType::Camera),
            mic_muted: self.stream_muted(proto::StreamType::Mic),
            screen_muted: self.stream_muted(proto::StreamType::Screen),
        }
    }

    fn stream_muted(&self, stype: proto::StreamType) -> Option<bool> {
        self.state.streams
            .iter()
            .find(|x|x.1.stype == stype.value())
            .map(|x|x.1.muted)
                
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum UserStage {
    Init,
    Ready,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct UserBrief {
    camera_muted: Option<bool>,
    mic_muted: Option<bool>,
    screen_muted: Option<bool>,
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnOpenedArgs {
    pub session_id: String,
    // pub create_x_responses: Option<Vec<records::OnCreateXResponse>>,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnClosedArgs {
    pub status: records::Status,
}




#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnCreatedXArgs {
    pub dir: i32,
    pub response: records::OnCreateXResponse,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnRoomChatArgs {
    pub chat: records::ChatArgs,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUserChatArgs {
    pub chat: records::ChatArgs,
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUserJoinedArgs {
    pub user_id: String, 
    pub tree: Vec<records::TreeNode>,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUserLeavedArgs {
    pub user_id: String,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnAddStreamArgs {
    pub user_id: String,
    pub stype: records::StreamType, 
    pub muted: bool,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUpdateStreamArgs {
    pub user_id: String,
    pub stype: records::StreamType, 
    pub muted: bool,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnRemoveStreamArgs {
    pub user_id: String,
    pub stype: records::StreamType, 
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUpdateUserTreeArgs {
    pub user_id: String,
    pub node: records::TreeNode, 
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnUpdateRoomTreeArgs {
    pub node: records::TreeNode, 
}



pub struct ResolveFn<F>(pub F);

impl<F> CbResolveConnX for ResolveFn<F> 
where 
    F: Fn(records::ConnXReturn) -> ForeignResult<()> + Send + Sync,
{
    fn resolve(&self, response:records::ConnXReturn) -> ForeignResult<()>  {
        self.0(response)
    }
}

impl<F> CbResolvePub for ResolveFn<F> 
where 
    F: Fn(records::PubReturn) -> ForeignResult<()> + Send + Sync,
{
    fn resolve(&self, response:records::PubReturn) -> ForeignResult<()>  {
        self.0(response)
    }
}

impl<F> CbResolveSub for ResolveFn<F> 
where 
    F: Fn(records::SubReturn) -> ForeignResult<()> + Send + Sync,
{
    fn resolve(&self, response: records::SubReturn) -> ForeignResult<()>  {
        self.0(response)
    }
}

impl<F> CbResolveUSub for ResolveFn<F> 
where 
    F: Fn(records::UnsubReturn) -> ForeignResult<()> + Send + Sync,
{
    fn resolve(&self, response: records::UnsubReturn) -> ForeignResult<()>  {
        self.0(response)
    }
}


#[allow(unused)]
#[uniffi::export(with_foreign)]
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait Listener: Send + Sync + 'static {
    fn on_opened(&self, args: OnOpenedArgs) -> ForeignResult<()>;
    fn on_closed(&self, args: OnClosedArgs) -> ForeignResult<()>;
    
    fn on_created_x(&self, args: OnCreatedXArgs) -> ForeignResult<()>;
    fn on_room_chat(&self, args: OnRoomChatArgs) -> ForeignResult<()>;
    fn on_user_chat(&self, args: OnUserChatArgs) -> ForeignResult<()>;
    // fn on_user_ready(&self) -> ForeignResult<()> ;
    fn on_room_ready(&self) -> ForeignResult<()> ;

    // fn on_user_joined(&self, user_id: &str, camera_muted: Option<bool>, mic_muted: Option<bool>, screen_muted: Option<bool>, tree: Vec<proto::TreeState>) -> ForeignResult<()> {
    //     Ok(())
    // }

    fn on_user_joined(&self, args: OnUserJoinedArgs) -> ForeignResult<()>;

    fn on_user_leaved(&self, args: OnUserLeavedArgs) -> ForeignResult<()> ;

    fn on_add_stream(&self, args: OnAddStreamArgs) -> ForeignResult<()> ;

    fn on_update_stream(&self, args: OnUpdateStreamArgs) -> ForeignResult<()> ;

    fn on_remove_stream(&self, args: OnRemoveStreamArgs) -> ForeignResult<()> ;

    fn on_update_user_tree(&self, args: OnUpdateUserTreeArgs) -> ForeignResult<()>;

    fn on_update_room_tree(&self, args: OnUpdateRoomTreeArgs) -> ForeignResult<()>;



    // fn on_extra_sub(&self, user_id: String, xid: String, stream_id: String, stype: i32, producer_id: String, consumer_id: String, rtp: Option<String>) -> ForeignResult<()> {
    //     Ok(())
    // }

}

// #[allow(unused)]
// pub trait ListenerMut: Send + Sync + 'static {
//     fn on_opened(&mut self, session_id: &str) -> Result<()>;
//     fn on_closed(&mut self, status: proto::Status) -> Result<()>;
//     fn on_room_chat(&mut self, from: &str, to: &str, body: &str) -> Result<()>;
//     fn on_user_chat(&mut self, from: &str, to: &str, body: &str) -> Result<()>;
//     fn on_user_ready(&mut self) -> Result<()> {
//         Ok(())
//     }
//     fn on_room_ready(&mut self) -> Result<()> {
//         Ok(())
//     }

//     // fn on_user_joined(&mut self, user_id: &str, camera_muted: Option<bool>, mic_muted: Option<bool>, screen_muted: Option<bool>, tree: Vec<proto::TreeState>) -> Result<()> {
//     //     Ok(())
//     // }

//     fn on_user_joined(&mut self, user_id: AStr, tree: Vec<proto::TreeState>) -> Result<()> {
//         Ok(())
//     }

//     fn on_user_leaved(&mut self, user_id: AStr) -> Result<()> {
//         Ok(())
//     }

//     fn on_add_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> Result<()> {
//         Ok(())
//     }

//     fn on_update_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> Result<()> {
//         Ok(())
//     }

//     fn on_remove_stream(&mut self, user_id: AStr, stype: i32) -> Result<()> {
//         Ok(())
//     }

//     fn on_update_user_tree(&mut self, user_id: AStr, op: proto::TreeState) -> Result<()> {
//         Ok(())
//     }

//     fn on_update_room_tree(&mut self, op: proto::TreeState) -> Result<()> {
//         Ok(())
//     }



//     fn on_extra_sub(&mut self, user_id: String, xid: String, stream_id: String, stype: i32, producer_id: String, consumer_id: String, rtp: Option<String>) -> Result<()> {
//         Ok(())
//     }

// }
