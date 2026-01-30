use anyhow::{anyhow, Context, Result};
use const_format::formatcp;
use log::{debug, error, info, warn};

use parking_lot::Mutex;
use tokio::{sync::mpsc, task::JoinHandle, time::timeout};
// use mp_common_rs::prost::Message as PbMsg;
use tokio_tungstenite::tungstenite::http::Uri;

use tracing::{Span, instrument};

use trace_error::anyhow::trace_result;

use std::{marker::PhantomData, collections::{HashMap, VecDeque}, sync::Arc, time::{Duration, Instant}};

use crate::{client::{ws_tung::{TungConnector, TungStream}, xfer::Xfer}, kit::{astr::AStr, async_rt, root_span::root_span_child}, proto::{self}};




pub struct Client<T: Types> {
    task: JoinHandle<()>,
    tx: mpsc::Sender<()>,
    shared: Arc<Shared<T>>,
    _mark: PhantomData<T>,
}

impl<T: Types> Client<T> {

    #[trace_result]
    pub fn try_new<S>(url: S, span: Option<Span>, config: JoinConfig, listener: T::Listener) -> Result<Self> 
    where 
        S: Into<String>,
    {
        
        let url = url.into();

        let uri: Uri = url.parse()
            .with_context(|| format!("invalid url [{url}]"))?;

        let scheme = uri.scheme_str()
            .with_context(||"url has no scheme")?;

        if false
            || scheme.eq_ignore_ascii_case("ws") 
            || scheme.eq_ignore_ascii_case("wss") {
            // SchemeClient::Ws
        } else {
            return Err(anyhow!("unsupported scheme [{scheme}]"))
        };

        let url: AStr = url.into();

        let (tx, rx) = mpsc::channel(1);

        let shared: Arc<Shared<T>> = Arc::new(Shared {
            inbox: Mutex::new(VecDeque::new()),
        });

        let worker = Worker {
            connector: TungConnector::new(config.advance.connection.ignore_server_cert),
            response_handlers: HashMap::new(),
            users: HashMap::new(),
            shared: shared.clone(),
            config,
            url,
            rx,
            listener,

            extra: Default::default(),
        };

        let span = match span {
            Some(v) => v,
            None => root_span_child!("client-work"),
        };

        let task = async_rt::spawn_with_span(span,  async move {
            worker.run().await
        });


        Ok(Self {
            task,
            tx,
            shared,
            _mark: Default::default(),
        })
    }

    pub async fn into_finish(self) {
        drop(self.tx);
        let _r = self.task.await;
    }

    #[trace_result]
    pub fn request(
        &self, 
        // typ: proto::PacketType,
        body: String,
        handler: T::ResponseHandler,
    ) -> Result<()> 
    {
        let msg = Op::Request(OpRequest {
            // typ,
            body,
            handler,
        });

        self.shared.inbox.lock().push_back(msg);

        let r = self.tx.try_send(());

        if let Err(e) = r {
            match e {
                mpsc::error::TrySendError::Full(_v) => {},
                mpsc::error::TrySendError::Closed(_v) => {
                    return Err(anyhow!("client already closed"))
                },
            }
        }

        Ok(())
    }

}


#[derive(Default)]
struct Shared<T: Types> {
    inbox: Mutex<VecDeque<Op<T>>>,
}

impl<T: Types> Shared<T> {
    fn pop_inbox(&self) -> Option<Op<T>> {
        self.inbox.lock().pop_front()
    }
}

#[derive(Debug, Default)]
struct Extra {
    dlink_xsn: Option<i64>,
    dlink_xid: Option<String>,
    subs: HashMap<i64, ExtraSub>,
    users: HashMap<String, ()>,
}

#[derive(Debug)]
struct ExtraSub {
    user_id: String,
    stream_id: String,
    stream: proto::Stream,
}

struct Worker<T: Types> {
    url: AStr,
    config: JoinConfig,
    connector: TungConnector,
    rx: mpsc::Receiver<()>,
    shared: Arc<Shared<T>>,
    listener: T::Listener,
    response_handlers: HashMap<i64, T::ResponseHandler>,
    users: HashMap<AStr, UserCell>,

    extra: Extra,
}

impl<T: Types> Worker<T> {

    #[trace_result]
    async fn run(mut self) {

        let r = self.try_run().await;

        let status = match r {
            Ok(_v) => {
                debug!("finished Ok");
                proto::Status::default()
            },
            Err(e) => {
                let err = trace_error!("finished error", e);
                error!("{}", fmt_error!(err));

                let status: proto::Status = err.into();
                status
            },
        };

        let r = self.listener.on_closed(status);

        if let Err(e) = r {
            error!("{}", trace_fmt!("on_closed failed", e));
        }
    }

    #[trace_result]
    async fn try_run(&mut self) -> Result<()> {

        let mut session = self.phase_open().await?;

        // while !self.rx.is_closed() {
        loop {
            let r = self.phase_work(&mut session).await;

            if self.rx.is_closed() {
                //  把错误往上抛
                r?;
            } else {
                if let Err(e) = r {
                    error!("{}", trace_fmt!("phase_work failed", e));
                }
            }

            self.phase_reconnect(&mut session).await?;
        }

        // Ok(())
    }

    #[trace_result]
    #[instrument(skip(self, session))]
    async fn phase_work(&mut self, session: &mut Session) -> Result<()> {
        let mut rraw = RecvRaw::default();

        loop {
            self.try_handle_op(session).await.with_context(||"try_handle_msgs failed")?;
            
            let r = self.recv_packet(&mut session.conn, &mut rraw).await.with_context(||"recv_packet falied")?;

            let Some(packet) = r else {
                continue;
            };

            if let Some(ack_sn) = packet.ack {
                session.xfer.update_recv_ack(ack_sn);
            }

            if let Some(sn) = packet.sn() {
                session.xfer.update_recv_sn(sn);
            }

            let Some(pkt_type) = packet.typ() else {
                return Ok(())
            };
            
            match pkt_type {
                proto::PacketType::Response => {
                    let typed = packet.parse_as_server_rsp()?;
                    let ack = packet.ack.with_context(||"no sn field in response")?;
                    self.handle_response(session, ack, typed).await?;
                },

                proto::PacketType::Push0
                | proto::PacketType::Push1
                | proto::PacketType::Push2
                => {
                    let typed = packet.parse_as_server_push()?;
                    self.handle_server_push(session, typed).await?;
                }

                _ => {
                    return Err(anyhow!("unknown packet {:?}", packet));
                }
            };

        }

        // Ok(())
    }

    #[trace_result]
    #[instrument(skip(self, session, push))]
    async fn handle_server_push(&mut self, session: &mut Session, push: proto::ServerPush) -> Result<()> {
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
                    self.listener.on_room_chat(from_user_id, room_id, &notice.body)?;

                } else if let Some(to_user_id) = notice.to.strip_prefix(USER_PREFIX) {
                    self.listener.on_user_chat(from_user_id, to_user_id, &notice.body)?;

                } else {
                    warn!("unknown chat.to [{}]", notice.to);
                    return Ok(());
                }
            },
            proto::ServerPushType::UInit(id) => {
                let user_id: AStr = id.id.into();
                if let Some(cell) = self.users.remove(&user_id) {
                    debug!("got user init but has user [{cell:?}]");
                    self.listener.on_user_leaved(user_id)?;
                }
            },
            proto::ServerPushType::UReady(id) => {
                let user_id: AStr = id.id.into();

                let Some(user) = self.users.get_mut(&user_id) else {
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
                let tree = core::mem::replace(&mut user.tree, Vec::new());
                let brief = user.brief();
                self.listener.on_user_joined(user_id.clone(), tree)?;

                if let Some(muted) = brief.camera_muted {
                    self.listener.on_add_stream(user_id.clone(), proto::StreamType::Camera.value(), muted)?;
                }
                
                if let Some(muted) = brief.mic_muted {
                    self.listener.on_add_stream(user_id.clone(), proto::StreamType::Mic.value(), muted)?;
                }

                if let Some(muted) = brief.screen_muted {
                    self.listener.on_add_stream(user_id.clone(), proto::StreamType::Screen.value(), muted)?;
                }

                self.handle_extra_user(session, &user_id).await?;
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
                                    self.listener.on_user_leaved(user_id.clone())?;
                                    return Ok(())
                                }

                                if new_state.inst_id != cell.state.inst_id {
                                    // 不应该走到这里
                                    warn!("got new user instance, old [{}], new [{}]", cell.state.inst_id, new_state.inst_id);
                                    self.listener.on_user_leaved(user_id.clone())?;
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
                        cell.tree.push(tree_state);
                    },
                    UserStage::Ready => {
                        self.listener.on_update_user_tree(user_id, tree_state)?;
                    },
                }
            },
            proto::ServerPushType::RTree(tree_state) => {
                self.listener.on_update_room_tree(tree_state)?;
            },
            proto::ServerPushType::RReady(_id) => {
                self.listener.on_room_ready()?;
            },
        }
        Ok(())
    }

    #[trace_result]
    #[instrument(skip(self, session, response))]
    async fn handle_response(&mut self, session: &mut Session, ack: i64, mut response: proto::ServerResponse) -> Result<()> {
        {
            let handled = self.handle_extra_response(session, ack, &response).await?;
            if handled {
                return Ok(())
            }
        }

        let Some(mut handler) = self.response_handlers.remove(&ack) else {
            return Err(anyhow!("Not found response handler, ack [{ack}]"))
        };

        let status = response.status.take().unwrap_or_default();
        
        if status.code == 0 {

            handler.on_success(response.typ.with_context(||"no typ field in response")?)?;
        } else {
            handler.on_fail(status.code, status.reason)?;
        }

        Ok(())
    }

    #[trace_result]
    async fn try_handle_op(&mut self, session: &mut Session) -> Result<()> {
        while let Some(msg) = self.shared.pop_inbox() {
            self.handle_op(session, msg).await?;
        }

        for packet in session.xfer.send_iter() {
            session.conn.send_packet(packet).await?;
        }

        for item in session.xfer.ack_iter() {
            let packet = item?;
            session.conn.send_packet(packet).await?;
        }

        Ok(())
    }

    #[trace_result]
    async fn handle_op(&mut self, session: &mut Session, msg: Op<T>) -> Result<()> {
        match msg {
            Op::Request(op) => {
                let sn = session.xfer.add_qos1_json(proto::PacketType::Request, &op.body, None)?;
                self.response_handlers.insert(sn, op.handler);

                self.handle_extra_request(session, sn, &op.body).await?
            }
        }
        Ok(())
    }

    #[trace_result]
    async fn handle_extra_request(&mut self, _session: &mut Session, sn: i64, body: &str) -> Result<()> {
        let request: proto::ClientRequest = serde_json::from_str(body)?;
        let typ = request.typ.with_context(||"no typ field")?;
        match typ {
            proto::client_message::MsgType::CreateX(req) => {
                if req.dir == proto::Direction::Outbound as i32 {
                    self.extra.dlink_xsn = Some(sn);
                    debug!("send extra dlink xsn [{sn}]");
                }
            },
            _ => {}
        }
        Ok(())
    }

    #[trace_result]
    async fn handle_extra_response(&mut self, session: &mut Session, ack: i64, response: &proto::ServerResponse) -> Result<bool> {

        let typ = response.typ.as_ref().with_context(||"no typ field")?;
        match typ {
            proto::response::ResponseType::CreateX(rsp) => {
                if Some(ack) == self.extra.dlink_xsn {
                    debug!("got extra dlink xid [{}], ack [{ack}]", rsp.xid);
                    self.extra.dlink_xid = Some(rsp.xid.clone());
                    self.send_extra_users(session).await?;
                    return Ok(false)
                }
            },
            proto::response::ResponseType::Sub(rsp) => {
                if let Some(xid) = &self.extra.dlink_xid {
                    if let Some(sub) = self.extra.subs.remove(&ack) {
                        let rtp = match &rsp.rtp {
                            Some(rtp) => {
                                Some(serde_json::to_string(&rtp)?)
                            },
                            None => None,
                        };

                        debug!("got extra sub {sub:?}");
                        self.listener.on_extra_sub(sub.user_id, xid.clone(), sub.stream_id, sub.stream.stype, sub.stream.producer_id, rsp.consumer_id.clone(), rtp)?;
                        return Ok(true)
                    }
                }
            },
            _ => {
                // warn!("expect extra dlink response CreateX but {response:?}");
            },
        }

        Ok(false)
    }

    #[trace_result]
    async fn handle_extra_user(&mut self, session: &mut Session, user_id: &str) -> Result<()> {

        self.extra.users.insert(user_id.into(), ());

        self.send_extra_users(session).await?;

        Ok(())

        // let Some(user) = self.users.get(user_id) else {
        //     return Ok(())
        // };

        // let Some(xid) = &self.extra.dlink_xid else {
        //     return Ok(())
        // };

        // for (stream_id, stream) in user.state.streams.iter() {
        //     let req = proto::SubscribeRequest {
        //         room_id: "".into(),
        //         stream_id: stream_id.clone(),
        //         producer_id: stream.producer_id.clone(), 
        //         xid: xid.clone(), 
        //         preferred_layers: None,
        //     };

        //     let body = serde_json::to_string(&proto::ClientRequest { typ: Some(proto::client_message::MsgType::Sub(req)) })?;

        //     let sn = session.xfer.add_qos1_json(proto::PacketType::Request, &body, None)?;

        //     self.extra.subs.insert(sn, ExtraSub {
        //         user_id: user_id.into(),
        //         stream_id: stream_id.clone(),
        //         stream: stream.clone(),
        //     });

        //     debug!("send extra sub stream {stream:?}, sn [{sn}]");
        // }
        // Ok(())
    }

    #[trace_result]
    async fn send_extra_users(&mut self, session: &mut Session) -> Result<()> {

        let Some(xid) = &self.extra.dlink_xid else {
            return Ok(())
        };

        let users = core::mem::replace(&mut self.extra.users, Default::default());
        for (user_id, v) in users {
            let Some(user) = self.users.get(user_id.as_str()) else {
                self.extra.users.insert(user_id, v);
                continue;
            };

            for (stream_id, stream) in user.state.streams.iter() {
                let req = proto::SubscribeRequest {
                    room_id: "".into(),
                    stream_id: stream_id.clone(),
                    producer_id: stream.producer_id.clone(), 
                    xid: xid.clone(), 
                    preferred_layers: None,
                };

                let body = serde_json::to_string(&proto::ClientRequest { typ: Some(proto::client_message::MsgType::Sub(req)) })?;

                let sn = session.xfer.add_qos1_json(proto::PacketType::Request, &body, None)?;

                self.extra.subs.insert(sn, ExtraSub {
                    user_id: user_id.clone().into(),
                    stream_id: stream_id.clone(),
                    stream: stream.clone(),
                });

                debug!("send extra sub stream {stream:?}, sn [{sn}], user_id [{user_id}]");
            }
        }


        Ok(())
    }


    #[trace_result]
    #[instrument(skip(self, _session))]
    async fn phase_reconnect(&mut self, _session: &mut Session) -> Result<()> {
        let (_conn, _raw) = timeout(
            self.config.advance.connection.max_timeout(), 
            self.reconnect_loop(),
        )
        .await
        .map_err(|_e|anyhow!(
            "reach max timeout [{:?}]", 
            self.config.advance.connection.max_timeout())
        )??;
        Ok(())
    }

    #[trace_result]
    #[instrument(skip(self))]
    async fn phase_open(&mut self) -> Result<Session> {

        // 把执行 open_session 的时间也考虑到 max_timeout 里

        let (conn, raw) = timeout(
            self.config.advance.connection.max_timeout(), 
            self.open_loop(),
        )
        .await
        .map_err(|_e|anyhow!(
            "reach max timeout [{:?}]", 
            self.config.advance.connection.max_timeout())
        )??;

        let packet: proto::PacketRef = serde_json::from_str(raw.as_str())
            .with_context(||"recv raw but invalid packet")?;

        let rsp = parse_open_response_packet(&packet).with_context(||"parse_open_response_packet failed")?;

        // let rsp = output.into_rsp().with_context(||"into_rsp falied")?;

        let session = Session {
            session_id: rsp.session_id,
            conn_id: rsp.conn_id,
            xfer: Xfer::new(),
            conn,
        };

        info!("opened session [{}], conn_id [{}]", session.session_id, session.conn_id);

        let r = self.listener.on_opened(&session.session_id);
        if let Err(e) = r {
            error!("{}", trace_fmt!("on_opened failed", e));
        }

        Ok(session)

    }

    #[trace_result]
    async fn open_loop<'a>(&mut self) -> Result<(Conn, PacketRaw)> {
        loop {

            let mut conn = self.connect_loop().await?;

            let r = self.open_session(&mut conn).await;

            match r {
                Ok(raw) => return Ok((conn, raw)),
                Err(e) => {
                    warn!("{}", trace_fmt!("open_session failed", e));
                    debug!("will retry opening session in [{:?}]", self.config.advance.connection.retry_interval());
                    self.sleep(self.config.advance.connection.retry_interval()).await?;
                    continue;
                },
            };
        }

    }

    #[trace_result]
    async fn reconnect_loop<'a>(&mut self) -> Result<(Conn, PacketRaw)> {
        loop {

            let mut conn = self.connect_loop().await?;

            let r = self.reconnect_session(&mut conn).await;

            match r {
                Ok(raw) => return Ok((conn, raw)),
                Err(e) => {
                    warn!("{}", trace_fmt!("reconnect_session failed", e));
                    debug!("will retry opening session in [{:?}]", self.config.advance.connection.retry_interval());
                    self.sleep(self.config.advance.connection.retry_interval()).await?;
                    continue;
                },
            };
        }

    }

    #[trace_result]
    async fn reconnect_session<'a>(&mut self, _conn: &mut Conn) -> Result<PacketRaw> {
        async_rt::sleep(Duration::from_secs(99999)).await;
        Err(anyhow!("Not implement"))
    }

    #[trace_result]
    async fn connect_loop(&mut self) -> Result<Conn> {


        loop {
            
            debug!("connecting to [{}]...", self.url);
            
            let r = self.connect_to().await;

            let cfg = &self.config.advance.connection;

            match r {
                Ok(conn) => {
                    debug!("connected to [{}]", self.url);
                    return Ok(conn)
                },
                Err(e) => {
                    warn!("{}", trace_fmt!("connect failed", e))
                }
            }

            debug!("will retry connecting in [{:?}]", cfg.retry_interval());
            self.sleep(cfg.retry_interval()).await?;
        }
    }

    #[trace_result]
    async fn connect_to(&mut self) -> Result<Conn> {
        let cfg = &self.config.advance.connection;

        loop {
            let r = tokio::select! {
                r = self.rx.recv() => {
                    check_guard(r)?;
                    continue;
                }

                r = timeout(cfg.connect_timeout(), self.connector.connect(&self.url)) => {
                    r
                }
            };

            let stream = r
                .map_err(|_e|anyhow!(
                    "reach connect timeout [{:?}]", 
                    cfg.connect_timeout())
                )??;

            return Ok(Conn {
                    stream,
                })
        }
    }


    #[trace_result]
    async fn open_session<'a>(&mut self, conn: &mut Conn) -> Result<PacketRaw> {
        
        self.open_session_send(conn).await.with_context(||"open_session_send failed")?;

        loop {
            let r = self.recv_raw(conn).await.with_context(||"recv_raw failed")?;
            if let Some(raw) = r {
                return Ok(raw)
            }
        }
    }


    #[trace_result]
    async fn open_session_send(&mut self, conn: &mut Conn) -> Result<()> {
        use crate::proto::IntoIterSerialize;

        let client_info = self.config.advance.client_info.as_ref().map(|ci| {
            let device: Option<std::collections::HashMap<&str, &str>> = ci.device.as_ref().map(|m| {
                m.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
            });
            let sdk = if ci.sdk_name.is_some() || ci.sdk_version.is_some() {
                Some(proto::SdkInfoSer {
                    name: ci.sdk_name.as_deref(),
                    version: ci.sdk_version.as_deref(),
                })
            } else {
                None
            };
            proto::ClientInfoSer {
                platform: ci.platform.as_deref(),
                sdk,
                device,
            }
        });

        let req = proto::OpenSessionRequestSer {
            user_id: &self.config.user_id,
            room_id: &self.config.room_id,
            client_info,
            token: self.config.advance.token.as_deref(),
            user_ext: self.config.advance.user_ext.as_deref(),
            user_tree: self.config.advance.user_tree
                .as_ref()
                .map(|x|
                    x.iter().map(|y|proto::UpdateTreeRequestSer::from(y))
                    .into_iter_ser()
                ),
            batch: self.config.advance.batch,
        };

        let packet = proto::PacketSer {
            typ: Some(proto::PacketType::Request.as_num()),
            sn: None, // Open 请求可以没有 sn
            body: Some(req.into_body()),
            ack: None, // 
        };

        conn.send_packet_ser(&packet).await
            .with_context(||"send_packet failed")?;

        Ok(())
    }


    #[trace_result]
    async fn recv_packet<'a>(&mut self, conn: &mut Conn, rraw: &'a mut RecvRaw) -> Result<Option<proto::PacketRef<'a>>> {
        loop {
            let r = self.recv_raw(conn).await?;

            let Some(raw) = r else {
                return Ok(None)
            };

            rraw.raw = raw;

            let packet: proto::PacketRef = serde_json::from_str(&rraw.raw.as_str()).with_context(||"parse packet failed")?;

            return Ok(Some(packet))
        }
        
        // Ok(None)
    }

    #[trace_result]
    async fn recv_raw<'a>(&mut self, conn: &mut Conn) -> Result<Option<PacketRaw>> {
        loop {
            tokio::select! {
                r = self.rx.recv() => {
                    check_guard(r)?;
                    return Ok(None)
                }

                r = conn.stream.wait_next_recv() => {
                    r.with_context(||"wait_next_recv failed")?;
                }
            }
            
            let r = conn.stream.recv_next_packet().await.with_context(||"wait_next_recv failed")?;
            
            let Some(raw) = r else {
                continue;
            };

            debug!("recv raw [{}]", raw.as_str());

            return Ok(Some(raw))
        }

        // Ok(None)
    }

    #[trace_result]
    async fn sleep(&mut self, d: Duration) -> Result<bool> {

        let deadline = Instant::now() + d;

        loop {
            tokio::select! {
                r = self.rx.recv() => {
                    check_guard(r)?;
                }

                _r = async_rt::sleep_until(deadline) => {
                    return Ok(true)
                }
            };
        }
    }
}


fn check_guard(r: Option<()>) -> Result<()> {
    r.with_context(||"got closed")
}


fn parse_open_response_packet(packet: &proto::PacketRef) -> Result<proto::OpenSessionResponse> {
    if packet.typ() != Some(proto::PacketType::Response) {
        return Err(anyhow!("invalid packet type {:?}", packet.typ()))
    }

    let body = packet.body.as_ref().with_context(||"empty body")?;

    let mut server_rsp: proto::ServerResponse = serde_json::from_str(body).with_context(||"invalid json")?;

    if let Some(status) = server_rsp.status.take() {
        return status.with_context(||"open response status")
    }

    let rsp_type = server_rsp.typ.with_context(||"invalid response type")?;
    match rsp_type {
        proto::response::ResponseType::Open(rsp) => {
            Ok(rsp)
            // Ok(Session {
            //     session_id: rsp.session_id,
            //     conn_id: rsp.conn_id,
            //     xfer: Xfer::new(),
            // })
        },
        _ => {
            Err(anyhow!("unexpect response type"))
        }
    }
}

struct Session {
    session_id: String,
    conn_id: String,
    xfer: Xfer,
    conn: Conn,
}

#[derive(Debug)]
struct UserCell {
    user_id: AStr,
    state: proto::UserState,
    tree: Vec<proto::TreeState>,
    stage: UserStage,
}

impl UserCell {
    pub fn delta<L>(&self, new_state: &proto::UserState, listener: &mut L) -> Result<()>
    where 
        L: Listener,
    {
        let old_state = &self.state;

        for (stream_id, old_stream) in old_state.streams.iter() {
            match new_state.streams.get(stream_id) {
                Some(new_stream) => {
                    if old_stream.producer_id != new_stream.producer_id {
                        listener.on_remove_stream(self.user_id.clone(), old_stream.stype)?;
                        listener.on_add_stream(self.user_id.clone(), new_stream.stype, new_stream.muted)?;
                    } else {
                        if old_stream.muted != new_stream.muted {
                            listener.on_update_stream(self.user_id.clone(), old_stream.stype, new_stream.muted)?;
                        }
                    }
                },
                None => {
                    listener.on_remove_stream(self.user_id.clone(), old_stream.stype)?;
                },
            }
        }

        for (stream_id, new_stream) in new_state.streams.iter() {

            match old_state.streams.get(stream_id) {
                Some(_old_stream) => {},
                None => {
                    listener.on_add_stream(self.user_id.clone(), new_stream.stype, new_stream.muted)?;
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



#[derive(Debug, Default)]
struct RecvRaw {
    raw: PacketRaw,
}

struct Conn {
    stream: TungStream,
}

impl Conn {
    pub async fn send_packet_ser<B>(&mut self, packet: &proto::PacketSer<B>) -> Result<()> 
    where
        B: serde::Serialize,
    {
        let json = serde_json::to_string(&packet)?;
        self.send_packet(json).await
    }

    pub async fn send_packet(&mut self, text: String) -> Result<()> {
        debug!("send raw [{}]", text);
        self.stream.send_text(text).await?;
        Ok(())
    }

}

type PacketRaw = tokio_tungstenite::tungstenite::Utf8Bytes;




#[derive(Debug, Clone)]
pub struct JoinConfig {
    pub user_id: AStr,

    pub room_id: AStr,

    pub advance: JoinAdvanceArgs,
}

#[derive(Debug, Clone, Default)]
pub struct JoinAdvanceArgs {
    pub user_ext: Option<AStr>,

    pub user_tree: Option<Vec<proto::UpdateTreeRequest>>, 

    pub connection: ConnectionConfig,

    pub batch: Option<bool>,

    pub token: Option<AStr>,

    pub client_info: Option<ClientInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub platform: Option<AStr>,
    pub sdk_name: Option<AStr>,
    pub sdk_version: Option<AStr>,
    pub device: Option<HashMap<AStr, AStr>>,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionConfig {
    /// 单次连接尝试的超时时间（每次 dial/syn 的超时）。
    /// `None` 表示使用默认值超时。
    pub connect_timeout: Option<Duration>,

    /// 所有重试的总共允许时长（从第一个尝试开始算起的截止时间）。
    /// `None` 表示使用默认值超时。
    pub max_timeout: Option<Duration>,

    /// 每次重试前等待的基础间隔（fixed interval）。
    /// `None` 表示使用默认值
    pub retry_interval: Option<Duration>,

    /// 忽略服务器的证书  
    pub ignore_server_cert: bool,

    // /// 禁止 nagle 算法
    // pub disable_nagle: bool,
}

impl ConnectionConfig {
    fn connect_timeout(&self) -> Duration {
        self.connect_timeout.unwrap_or(Duration::from_secs(5))
    }

    fn max_timeout(&self) -> Duration {
        self.max_timeout.unwrap_or(Duration::from_secs(10))
    }

    fn retry_interval(&self) -> Duration {
        self.retry_interval.unwrap_or(Duration::from_secs(1))
    }
}


pub trait Types: 'static {
    type Listener: Listener;
    type ResponseHandler: ResponseHandler;
}

impl Types for () {
    type Listener = ();
    type ResponseHandler = ();
}

#[allow(unused)]
pub trait Listener: Send + 'static {
    fn on_opened(&mut self, session_id: &str) -> Result<()>;
    fn on_closed(&mut self, status: proto::Status) -> Result<()>;
    fn on_room_chat(&mut self, from: &str, to: &str, body: &str) -> Result<()>;
    fn on_user_chat(&mut self, from: &str, to: &str, body: &str) -> Result<()>;
    

    // fn on_user_joined(&mut self, user_id: &str, camera_muted: Option<bool>, mic_muted: Option<bool>, screen_muted: Option<bool>, tree: Vec<proto::TreeState>) -> Result<()> {
    //     Ok(())
    // }

    fn on_user_joined(&mut self, user_id: AStr, tree: Vec<proto::TreeState>) -> Result<()> {
        Ok(())
    }

    fn on_user_leaved(&mut self, user_id: AStr) -> Result<()> {
        Ok(())
    }

    fn on_add_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> Result<()> {
        Ok(())
    }

    fn on_update_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> Result<()> {
        Ok(())
    }

    fn on_remove_stream(&mut self, user_id: AStr, stype: i32) -> Result<()> {
        Ok(())
    }

    fn on_update_user_tree(&mut self, user_id: AStr, op: proto::TreeState) -> Result<()> {
        Ok(())
    }

    fn on_update_room_tree(&mut self, op: proto::TreeState) -> Result<()> {
        Ok(())
    }

    fn on_room_ready(&mut self) -> Result<()> {
        Ok(())
    }

    fn on_extra_sub(&mut self, user_id: String, xid: String, stream_id: String, stype: i32, producer_id: String, consumer_id: String, rtp: Option<String>) -> Result<()> {
        Ok(())
    }

}

impl Listener for () {
    fn on_opened(&mut self, _session_id: &str) -> Result<()> {
        Ok(())
    }

    fn on_closed(&mut self, _status: proto::Status) -> Result<()> {
        Ok(())
    }
    
    fn on_room_chat(&mut self, _from: &str, _to: &str, _body: &str) -> Result<()> {
        Ok(())
    }
    
    fn on_user_chat(&mut self, _from: &str, _to: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}


pub trait ResponseHandler: 'static + Send {
    fn on_success(&mut self, response: proto::response::ResponseType)-> Result<()>;
    fn on_fail(&mut self, code: i32, reason: String) -> Result<()>;
}

impl ResponseHandler for Box<dyn ResponseHandler> {
    fn on_success(&mut self, response: proto::response::ResponseType)-> Result<()> {
        self.as_mut().on_success(response)
    }

    fn on_fail(&mut self, code: i32, reason: String) -> Result<()> {
        self.as_mut().on_fail(code, reason)
    }
}


impl ResponseHandler for () {
    fn on_success(&mut self, _response: proto::response::ResponseType) -> Result<()> {
        Ok(())
    }

    fn on_fail(&mut self, _code: i32, _reason: String) -> Result<()> {
        Ok(())
    }
}

enum Op<T: Types> {
    Request(OpRequest<T>),
}


struct OpRequest<T: Types> {
    body: String,
    handler: T::ResponseHandler,
}
