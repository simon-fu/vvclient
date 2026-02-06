
use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, warn};

use core::future::Future;

use tokio::{sync::mpsc, task::JoinHandle, time::timeout};
// use mp_common_rs::prost::Message as PbMsg;
use tokio_tungstenite::tungstenite::http::Uri;

use tracing::{Span, instrument};

use trace_error::anyhow::trace_result;

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::{client::{ws_tung::{TungConnector, TungStream}, xfer::Xfer}, kit::{astr::AStr, async_rt}, proto::{self}};

use super::defines::*;



pub struct Worker {
    task_handle: JoinHandle<()>,
    tx: mpsc::Sender<()>,
    shutdown_mode: Arc<AtomicU8>,
}

impl Worker {

    #[trace_result]
    pub fn try_new<S, T>(url: S, span: Option<Span>, config: JoinConfig, listener: T) -> Result<Self> 
    where 
        S: Into<String>,
        T: Delegate,
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
        let shutdown_mode = Arc::new(AtomicU8::new(ShutdownMode::None as u8));

        let task = Task {
            connector: TungConnector::new(config.advance.connection.ignore_server_cert),
            flag: Flag::default(),
            config,
            url,
            rx,
            listener,
            shutdown_mode: shutdown_mode.clone(),
        };

        let span = match span {
            Some(v) => v,
            None => tracing::span!(parent: None, tracing::Level::ERROR, "client"),
        };

        let task_handle = async_rt::spawn_with_span(span,  async move {
            task.run().await;
        });

        Ok(Self {
            task_handle,
            tx,
            shutdown_mode,
            // shared,
            // _mark: Default::default(),
        })
    }

    pub async fn into_finish(self) {
        drop(self.tx);
        let _r = self.task_handle.await;
    }

    #[trace_result]
    pub fn commit(&self) -> Result<()> {
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

    pub fn set_shutdown_mode(&self, force: bool) {
        let v = if force { ShutdownMode::Force } else { ShutdownMode::Graceful };
        self.shutdown_mode.store(v as u8, Ordering::Relaxed);
    }
}


struct Task<T: Delegate> {
    url: AStr,
    config: JoinConfig,
    connector: TungConnector,
    rx: mpsc::Receiver<()>,
    flag: Flag,
    listener: T,
    shutdown_mode: Arc<AtomicU8>,
}

impl<T: Delegate> Task<T> {

    #[trace_result]
    async fn run(mut self) {

        let r = self.try_run().await;

        let status = match r {
            Ok(_v) => {
                debug!("finished Ok");
                proto::Status::default()
            },
            Err(e) => {
                if let Some(status) = self.flag.got_closed.take() {
                    debug!("got closed");
                    status
                } else {
                    let err = trace_error!("finished error", e);
                    error!("{}", fmt_error!(err));
                    err.into()
                }
            },
        };

        let r = self.listener.on_closed(status).await;

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

            if self.flag.got_closed.is_some() {
                return Ok(())
            }

            if self.rx.is_closed() {
                //  把错误往上抛
                r?;
            } else {
                if let Err(e) = r {
                    let status = proto::Status::from(&e);
                    if status.code == 0 {
                        debug!("phase_work ended with no error status");
                        return Ok(())
                    }
                    
                    error!("{}", trace_fmt!("phase_work failed", e));
                }
            }

            self.phase_reconnect(&mut session).await?;
        }

        // Ok(())
    }

    #[trace_result]
    #[instrument(name="work", skip(self, session))]
    async fn phase_work(&mut self, session: &mut Session) -> Result<()> {
        let mut rraw = RecvRaw::default();

        loop {
            self.try_handle_op(session).await.with_context(||"try_handle_msgs failed")?;
            let hb_deadline = session.heartbeat_deadline(&self.config.advance.connection);

            let r = if let Some(deadline) = hb_deadline {
                tokio::select! {
                    r = self.recv_packet(&mut session.conn, &mut rraw, RecvCancelMode::Work) => {
                        r.with_context(||"recv_packet falied")?
                    }
                    _ = async_rt::sleep_until(deadline) => {
                        self.handle_heartbeat_tick(session).await?;
                        continue;
                    }
                }
            } else {
                self.recv_packet(&mut session.conn, &mut rraw, RecvCancelMode::Work)
                    .await
                    .with_context(||"recv_packet falied")?
            };

            let Some(packet) = r else {
                if RecvCancelMode::Work.should_break(self.shutdown_mode()) {
                    return Ok(())
                }
                continue;
            };

            session.on_recv();

            if let Some(ack_sn) = packet.ack {
                session.xfer.update_recv_ack(ack_sn);
            }

            if let Some(sn) = packet.sn() {
                session.xfer.update_recv_sn(sn);
            }

            let Some(pkt_type) = packet.typ() else {
                continue;
            };
            
            match pkt_type {
                proto::PacketType::Response => {
                    let ack = packet.ack.with_context(||"no ack field in response")?;
                    let mut typed = packet.parse_as_server_rsp()?;
                    if session.try_handle_heartbeat_response(ack, &mut typed)? {
                        continue;
                    }
                    self.listener.on_response(session, ack, typed).await?;
                    // self.handle_response(session, ack, typed).await?;
                },

                proto::PacketType::Push0
                | proto::PacketType::Push1
                | proto::PacketType::Push2
                => {
                    let typed = packet.parse_as_server_push()?;
                    if let proto::ServerPushType::Closed(notice) = typed.typ {
                        self.flag.got_closed = Some(notice.status.unwrap_or_default());
                        return Ok(())
                    }
                    self.listener.on_push(session, typed).await?;
                }

                _ => {
                    return Err(anyhow!("unknown packet {:?}", packet));
                }
            };

        }

        // Ok(())
    }

    // #[trace_result]
    // #[instrument(skip(self, session, push))]
    // async fn handle_server_push(&mut self, session: &mut Session, push: proto::ServerPush) -> Result<()> {
    //     match push.typ {
    //         proto::ServerPushType::Closed(notice) => {
    //             return Err(notice.status.unwrap_or_default())
    //         },
    //         proto::ServerPushType::Chat(notice) => {
    //             const CHAT_DIV: char = '.';
    //             const USER_PREFIX: &'static str = formatcp!("u{CHAT_DIV}"); 
    //             const ROOM_PREFIX: &'static str = formatcp!("r{CHAT_DIV}"); 

    //             let Some(from_user_id) = notice.from.strip_prefix(USER_PREFIX) else {
    //                 warn!("unknown chat.from [{}]", notice.from);
    //                 return Ok(());
    //             };

    //             if let Some(room_id) = notice.to.strip_prefix(ROOM_PREFIX) {
    //                 self.listener.on_room_chat(from_user_id, room_id, &notice.body)?;

    //             } else if let Some(to_user_id) = notice.to.strip_prefix(USER_PREFIX) {
    //                 self.listener.on_user_chat(from_user_id, to_user_id, &notice.body)?;

    //             } else {
    //                 warn!("unknown chat.to [{}]", notice.to);
    //                 return Ok(());
    //             }
    //         },
    //         proto::ServerPushType::UInit(id) => {
    //             let user_id: AStr = id.id.into();
    //             if let Some(cell) = self.users.remove(&user_id) {
    //                 debug!("got user init but has user [{cell:?}]");
    //                 self.listener.on_user_leaved(user_id)?;
    //             }
    //         },
    //         proto::ServerPushType::UReady(id) => {
    //             let user_id: AStr = id.id.into();

    //             let Some(user) = self.users.get_mut(&user_id) else {
    //                 warn!("got user ready but NOT found user [{user_id}]");
    //                 return Ok(())
    //             };

    //             match &user.stage {
    //                 UserStage::Init => {},
    //                 UserStage::Ready => {
    //                     warn!("already ready but got user ready again, user_id [{user_id}]");
    //                 },
    //             }

    //             user.stage = UserStage::Ready;
    //             let tree = core::mem::replace(&mut user.tree, Vec::new());
    //             let brief = user.brief();
    //             self.listener.on_user_joined(user_id.clone(), tree)?;

    //             if let Some(muted) = brief.camera_muted {
    //                 self.listener.on_add_stream(user_id.clone(), proto::StreamType::Camera.value(), muted)?;
    //             }
                
    //             if let Some(muted) = brief.mic_muted {
    //                 self.listener.on_add_stream(user_id.clone(), proto::StreamType::Mic.value(), muted)?;
    //             }

    //             if let Some(muted) = brief.screen_muted {
    //                 self.listener.on_add_stream(user_id.clone(), proto::StreamType::Screen.value(), muted)?;
    //             }

    //             self.handle_extra_user(session, &user_id).await?;
    //         },
    //         proto::ServerPushType::UFull(new_state) => {
    //             let r = self.users.remove_entry(new_state.id.as_str());

    //             let (user_id, cell) = match r {
    //                 Some((user_id, mut cell)) => {
    //                     match &cell.stage {
    //                         UserStage::Init => {
    //                             if !new_state.online {
    //                                 return Ok(())
    //                             }
    //                         },
    //                         UserStage::Ready => {
    //                             if !new_state.online {
    //                                 // debug!("leaved user [{user_id}]");
    //                                 self.listener.on_user_leaved(user_id.clone())?;
    //                                 return Ok(())
    //                             }

    //                             if new_state.inst_id != cell.state.inst_id {
    //                                 // 不应该走到这里
    //                                 warn!("got new user instance, old [{}], new [{}]", cell.state.inst_id, new_state.inst_id);
    //                                 self.listener.on_user_leaved(user_id.clone())?;
    //                                 cell.stage = UserStage::Init;
    //                             } else {
    //                                 cell.delta(&new_state, &mut self.listener)?;    
    //                             }

    //                         },
    //                     }
    //                     cell.state = new_state;
    //                     (user_id, cell)
    //                 },
    //                 None => {
    //                     let user_id: AStr = new_state.id.clone().into();
    //                     (
    //                         user_id.clone(),
    //                         UserCell {
    //                             user_id,
    //                             state: new_state, 
    //                             tree: Default::default(), 
    //                             stage: UserStage::Init,
    //                         }
    //                     )
    //                 },
    //             };

    //             self.users.insert(user_id, cell);
    //         },
    //         proto::ServerPushType::UTree(mut tree_state) => {
    //             let user_id = tree_state.id.take().with_context(||"no user_id in user tree")?;
    //             let user_id: AStr = user_id.into();

    //             let Some(cell) = self.users.get_mut(&user_id) else {
    //                 warn!("got user tree but has no user [{user_id}]");
    //                 return Ok(())
    //             };

    //             match &cell.stage {
    //                 UserStage::Init => {
    //                     cell.tree.push(tree_state);
    //                 },
    //                 UserStage::Ready => {
    //                     self.listener.on_update_user_tree(user_id, tree_state)?;
    //                 },
    //             }
    //         },
    //         proto::ServerPushType::RTree(tree_state) => {
    //             self.listener.on_update_room_tree(tree_state)?;
    //         },
    //         proto::ServerPushType::RReady(_id) => {
    //             self.listener.on_room_ready()?;
    //         },
    //     }
    //     Ok(())
    // }

    // #[trace_result]
    // #[instrument(skip(self, session, response))]
    // async fn handle_response(&mut self, session: &mut Session, ack: i64, mut response: proto::ServerResponse) -> Result<()> {
    //     {
    //         let handled = self.handle_extra_response(session, ack, &response).await?;
    //         if handled {
    //             return Ok(())
    //         }
    //     }

    //     let Some(mut handler) = self.response_handlers.remove(&ack) else {
    //         return Err(anyhow!("Not found response handler, ack [{ack}]"))
    //     };

    //     let status = response.status.take().unwrap_or_default();
        
    //     if status.code == 0 {

    //         handler.on_success(response.typ.with_context(||"no typ field in response")?)?;
    //     } else {
    //         handler.on_fail(status.code, status.reason)?;
    //     }

    //     Ok(())
    // }

    #[trace_result]
    async fn try_handle_op(&mut self, session: &mut Session) -> Result<()> {
        // while let Some(msg) = self.shared.pop_inbox() {
        //     self.handle_op(session, msg).await?;
        // }

        self.listener.on_process(session).await.with_context(||"on_process failed")?;

        let _ = Self::flush_xfer(&mut session.conn, &mut session.xfer).await?;

        Ok(())
    }

    #[trace_result]
    async fn flush_xfer(conn: &mut Conn, xfer: &mut Xfer) -> Result<()> {
        for packet in xfer.send_iter() {
            conn.send_packet(packet).await?;
        }

        for item in xfer.ack_iter() {
            let packet = item?;
            conn.send_packet(packet).await?;
        }

        Ok(())
    }


    #[trace_result]
    #[instrument(name="reconnect", skip(self, _session))]
    async fn phase_reconnect(&mut self, _session: &mut Session) -> Result<()> {
        // NOTE:
        // Heartbeat is request/response only (no ack packet for HB response).
        // After reconnect succeeds, old pending heartbeat state must be cleared,
        // otherwise client may keep waiting for a lost pre-disconnect HB response.
        // reconnect_session should reset heartbeat state on the working Session.
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
    #[instrument(name="open", skip(self))]
    async fn phase_open(&mut self) -> Result<Session> {

        // 把执行 open_session 的时间也考虑到 max_timeout 里

        let (conn, raw, xfer) = timeout(
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

        let mut session = Session {
            session_id: rsp.session_id,
            conn_id: rsp.conn_id,
            xfer,
            conn,
            heartbeat_started: false,
            last_rx_at: Instant::now(),
            last_hb_at: Instant::now(),
            pending_hb_sn: None,
            // close_status: Default::default(),
        };

        session.start_heartbeat();

        info!("opened session [{}], conn_id [{}]", session.session_id, session.conn_id);

        let r = self.listener.on_opened(&mut session).await;
        if let Err(e) = r {
            error!("{}", trace_fmt!("on_opened failed", e));
        }

        Ok(session)

    }

    #[trace_result]
    async fn open_loop<'a>(&mut self) -> Result<(Conn, PacketRaw, Xfer)> {
        loop {

            let mut conn = self.connect_loop(RecvCancelMode::Connect).await?;

            let r = self.open_session(&mut conn).await;

            match r {
                Ok((raw, xfer)) => return Ok((conn, raw, xfer)),
                Err(e) => {
                    if RecvCancelMode::Connect.should_break(self.shutdown_mode()) {
                        return Err(e)
                    }
                    warn!("{}", trace_fmt!("open_session failed", e));
                    debug!("will retry opening session in [{:?}]", self.config.advance.connection.retry_interval());
                    if !self.sleep(self.config.advance.connection.retry_interval(), RecvCancelMode::Connect).await? {
                        return Err(anyhow!("open canceled"))
                    }
                    continue;
                },
            };
        }

    }

    #[trace_result]
    async fn reconnect_loop<'a>(&mut self) -> Result<(Conn, PacketRaw)> {
        loop {

            let mut conn = self.connect_loop(RecvCancelMode::Work).await?;

            let r = self.reconnect_session(&mut conn).await;

            match r {
                Ok(raw) => return Ok((conn, raw)),
                Err(e) => {
                    if RecvCancelMode::Work.should_break(self.shutdown_mode()) {
                        return Err(e)
                    }
                    warn!("{}", trace_fmt!("reconnect_session failed", e));
                    debug!("will retry opening session in [{:?}]", self.config.advance.connection.retry_interval());
                    if !self.sleep(self.config.advance.connection.retry_interval(), RecvCancelMode::Work).await? {
                        return Err(anyhow!("reconnect canceled"))
                    }
                    continue;
                },
            };
        }

    }

    #[trace_result]
    async fn reconnect_session<'a>(&mut self, _conn: &mut Conn) -> Result<PacketRaw> {
        // TODO:
        // 1. send Reconnect request and validate response.
        // 2. swap connection/session fields on success.
        // 3. reset heartbeat state for the reconnected session:
        //    - pending_hb_sn = None
        //    - last_hb_at = now
        //    - last_rx_at = now
        //    so stale HB response from old connection will not block new heartbeat loop.
        async_rt::sleep(Duration::from_secs(99999)).await;
        Err(anyhow!("Not implement"))
    }

    #[trace_result]
    async fn connect_loop(&mut self, mode: RecvCancelMode) -> Result<Conn> {

        if mode.should_break(self.shutdown_mode()) {
            return Err(anyhow!("connect canceled"))
        }

        loop {
            
            debug!("connecting to [{}]...", self.url);
            
            let r = self.connect_to(mode).await;

            let cfg = &self.config.advance.connection;

            match r {
                Ok(conn) => {
                    debug!("connected to [{}]", self.url);
                    return Ok(conn)
                },
                Err(e) => {
                    if mode.should_break(self.shutdown_mode()) {
                        return Err(e)
                    }
                    warn!("{}", trace_fmt!("connect failed", e))
                }
            }

            debug!("will retry connecting in [{:?}]", cfg.retry_interval());
            if !self.sleep(cfg.retry_interval(), mode).await? {
                return Err(anyhow!("connect canceled"))
            }
        }
    }

    #[trace_result]
    async fn connect_to(&mut self, mode: RecvCancelMode) -> Result<Conn> {
        let cfg = &self.config.advance.connection;

        loop {
            let r = tokio::select! {
                r = self.rx.recv() => {
                    self.flag.check_guard(r)?;
                    if mode.should_break(self.shutdown_mode()) {
                        return Err(anyhow!("connect canceled"))
                    }
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
    async fn open_session<'a>(&mut self, conn: &mut Conn) -> Result<(PacketRaw, Xfer)> {

        self.open_session_send(conn).await.with_context(||"open_session_send failed")?;

        let mut xfer = Xfer::new();
        self.listener.on_opening(&mut xfer).await?;
        let _ = Self::flush_xfer(conn, &mut xfer).await?;

        loop {
            let r = self.recv_raw(conn, RecvCancelMode::Connect).await.with_context(||"recv_raw failed")?;
            if let Some(raw) = r {
                return Ok((raw, xfer))
            }
            if RecvCancelMode::Connect.should_break(self.shutdown_mode()) {
                return Err(anyhow!("open canceled"))
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
            batch: self.config.advance.batch, // Some(true),
            // create_x_requests: Option::<()>::None,
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
    async fn recv_packet<'a>(&mut self, conn: &mut Conn, rraw: &'a mut RecvRaw, mode: RecvCancelMode) -> Result<Option<proto::PacketRef<'a>>> {
        loop {
            let r = self.recv_raw(conn, mode).await?;

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
    async fn recv_raw<'a>(&mut self, conn: &mut Conn, mode: RecvCancelMode) -> Result<Option<PacketRaw>> {
        loop {
            tokio::select! {
                r = self.rx.recv() => {
                    self.flag.check_guard(r)?;
                    if mode.should_break(self.shutdown_mode()) {
                        return Ok(None)
                    }
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
    async fn sleep(&mut self, d: Duration, mode: RecvCancelMode) -> Result<bool> {

        let deadline = Instant::now() + d;

        loop {
            tokio::select! {
                r = self.rx.recv() => {
                    self.flag.check_guard(r)?;
                    if mode.should_break(self.shutdown_mode()) {
                        return Ok(false)
                    }
                }

                _r = async_rt::sleep_until(deadline) => {
                    return Ok(true)
                }
            };
        }
    }

    fn shutdown_mode(&self) -> ShutdownMode {
        ShutdownMode::from_u8(self.shutdown_mode.load(Ordering::Relaxed))
    }

    #[trace_result]
    async fn handle_heartbeat_tick(&mut self, session: &mut Session) -> Result<()> {
        let cfg = &self.config.advance.connection;

        if !cfg.heartbeat_enable() {
            return Ok(())
        }

        if session.heartbeat_timed_out(cfg.heartbeat_timeout()) {
            return Err(anyhow!("heartbeat timeout"))
        }

        if session.heartbeat_due(cfg.heartbeat_interval()) {
            if !session.has_pending_heartbeat() {
                let body = proto::HeartbeatRequestSer {}.into_body();
                let sn = session.xfer_mut().add_qos1_json(proto::PacketType::Request, &body, None)?;
                session.set_pending_heartbeat(sn);
                session.on_heartbeat_sent();
                let _ = Self::flush_xfer(&mut session.conn, &mut session.xfer).await?;
            }
        }

        Ok(())
    }
}


#[derive(Debug, Clone, Default)]
struct Flag {
    got_closed: Option<proto::Status>,
}

impl Flag {
    fn check_guard(&mut self, r: Option<()>) -> Result<()> {
        let r = r.with_context(||"got closed");
        if r.is_err() {
            self.got_closed = Some(Default::default());
        }
        r
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShutdownMode {
    None = 0,
    Graceful = 1,
    Force = 2,
}

impl ShutdownMode {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => ShutdownMode::Graceful,
            2 => ShutdownMode::Force,
            _ => ShutdownMode::None,
        }
    }
}

#[derive(Clone, Copy)]
enum RecvCancelMode {
    Connect,
    Work,
}

impl RecvCancelMode {
    fn should_break(self, mode: ShutdownMode) -> bool {
        match self {
            RecvCancelMode::Connect => mode != ShutdownMode::None,
            RecvCancelMode::Work => mode == ShutdownMode::Force,
        }
    }
}



#[trace_result]
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

pub struct Session {
    session_id: String,
    conn_id: String,
    xfer: Xfer,
    conn: Conn,
    heartbeat_started: bool,
    last_rx_at: Instant,
    last_hb_at: Instant,
    pending_hb_sn: Option<i64>,
    // close_status: Option<proto::Status>,
}

impl Session {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    // pub fn set_close(&mut self, status: proto::Status) {
    //     self.close_status = Some(status);
    // }

    pub fn xfer_mut(&mut self) -> &mut Xfer {
        &mut self.xfer
    }

    fn start_heartbeat(&mut self) {
        let now = Instant::now();
        self.heartbeat_started = true;
        self.last_rx_at = now;
        self.last_hb_at = now;
        self.pending_hb_sn = None;
    }

    fn on_recv(&mut self) {
        self.last_rx_at = Instant::now();
    }

    fn on_heartbeat_sent(&mut self) {
        self.last_hb_at = Instant::now();
    }

    fn heartbeat_due(&self, interval: Duration) -> bool {
        self.heartbeat_started && self.last_hb_at.elapsed() >= interval
    }

    fn heartbeat_timed_out(&self, timeout: Duration) -> bool {
        self.heartbeat_started && self.last_rx_at.elapsed() >= timeout
    }

    fn heartbeat_deadline(&self, cfg: &super::defines::ConnectionConfig) -> Option<Instant> {
        if !cfg.heartbeat_enable() {
            return None
        }
        if !self.heartbeat_started {
            return None
        }
        Some(self.last_hb_at + cfg.heartbeat_interval())
    }

    fn has_pending_heartbeat(&self) -> bool {
        self.pending_hb_sn.is_some()
    }

    fn set_pending_heartbeat(&mut self, sn: i64) {
        self.pending_hb_sn = Some(sn);
    }

    fn try_handle_heartbeat_response(&mut self, ack: i64, response: &mut proto::ServerResponse) -> Result<bool> {
        let is_hb = matches!(
            response.typ.as_ref(),
            Some(proto::response::ResponseType::HB(_))
        );
        if !is_hb {
            return Ok(false)
        }

        let status = response.status.take().unwrap_or_default();
        if status.code != 0 {
            return Err(anyhow!("heartbeat failed: code {}, reason {}", status.code, status.reason))
        }

        if let Some(pending) = self.pending_hb_sn {
            if ack >= pending {
                self.pending_hb_sn = None;
            }
        }

        // HB response should always be consumed by heartbeat logic.
        Ok(true)
    }
}



#[derive(Debug, Default)]
struct RecvRaw {
    raw: PacketRaw,
}

struct Conn {
    stream: TungStream,
}

impl Conn {

    #[trace_result]
    pub async fn send_packet_ser<B>(&mut self, packet: &proto::PacketSer<B>) -> Result<()> 
    where
        B: serde::Serialize,
    {
        let json = serde_json::to_string(&packet)?;
        self.send_packet(json).await
    }

    #[trace_result]
    pub async fn send_packet(&mut self, text: String) -> Result<()> {
        debug!("send raw [{}]", text);
        self.stream.send_text(text).await?;
        Ok(())
    }

}

type PacketRaw = tokio_tungstenite::tungstenite::Utf8Bytes;









pub trait Delegate: Send + Sync + 'static {
    fn on_opening(&mut self, xfer: &mut Xfer) -> impl Future<Output = Result<()>> + Send;
    fn on_opened(&mut self, session: &mut Session) -> impl Future<Output = Result<()>> + Send;
    fn on_closed(&mut self, status: proto::Status) -> impl Future<Output = Result<()>> + Send;
    fn on_process(&mut self, session: &mut Session) -> impl Future<Output = Result<()>> + Send;
    fn on_response(&mut self, session: &mut Session, ack: i64, response: proto::ServerResponse) -> impl Future<Output = Result<()>> + Send;
    fn on_push(&mut self, session: &mut Session, push: proto::ServerPush) -> impl Future<Output = Result<()>> + Send;
}
