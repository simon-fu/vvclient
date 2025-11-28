use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, trace, warn};

use parking_lot::Mutex;
use tokio::{sync::{mpsc, oneshot}, task::JoinHandle, time::timeout};
// use mp_common_rs::prost::Message as PbMsg;
use tokio_tungstenite::tungstenite::http::Uri;

use tracing::{Span, instrument};

use trace_error::{WithLine, anyhow::trace_result};

use std::{collections::VecDeque, future::Future, sync::Arc, time::{Duration, Instant}};

use crate::{client::{ws_tung::{TungConnector, TungStream}, xfer::Xfer}, kit::{actor::{Action, ActionRes, Actor, ActorBuilder, ActorHandler, AsyncHandler}, astr::AStr, async_rt, root_span::root_span_child}, proto::{self, fmt_writer::JsonDisplay}};


pub trait Listener: Send + 'static {
    fn on_closed(&mut self, reason: proto::Status);
}

impl Listener for () {
    fn on_closed(&mut self, reason: proto::Status) {
        
    }
}

pub struct Client {
    // actor: Actor<Entity>,
    // tx: mpsc::Sender<()>,
    task: JoinHandle<()>,
    tx: mpsc::Sender<()>,
    shared: Arc<Shared>,
}

impl Client {

    #[trace_result]
    pub fn try_new<S, L>(url: S, span: Option<Span>, config: JoinConfig, listener: L) -> Result<Self> 
    where 
        S: Into<String>,
        L: Listener,
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

        let shared: Arc<Shared> = Default::default();

        let worker = Worker {
            connector: TungConnector::new(config.advance.connection.ignore_server_cert),
            shared: shared.clone(),
            config,
            url,
            rx,
            listener,
        };

        let span = match span {
            Some(v) => v,
            None => root_span_child!("client"),
        };

        let task = async_rt::spawn_with_span(span,  async move {
            worker.run().await
        });
        
        // let actor = ActorBuilder::new()
        //     .build(span, Entity {
        //         url,
        //         scheme,
        //         ws: Default::default(),
        //         msg_id: Default::default(),
        //         // notice_tx: Default::default(),
        //         // inflights: Default::default(),
        //     });

        Ok(Self {
            task,
            tx,
            shared,
        })
    }

    pub async fn into_finish(self) {
        drop(self.tx);
        let _r = self.task.await;
    }

    // pub async fn open(&self, biz_requests: Vec<BizRequest>, heartbeat: Option<i64>) -> Result<OpenOutput> {
    //     let output = self.actor.invoker().invoke(OpenInput {
    //         biz_requests,
    //         heartbeat,
    //     }).await??;
    //     Ok(output)
    // }

    // pub async fn reconnect(&self, req: ReconnectRequest) -> Result<ReconnectOutput> {
    //     let output = self.actor.invoker().invoke(ReconnectInput {
    //         req
    //     }).await??;
    //     Ok(output)
    // }

    // pub async fn close(&self) {
    //     let _r = self.actor.invoker().invoke(CloseCmd {}).await;
    // }


}

#[derive(Default)]
struct Shared {
    inbox: Mutex<VecDeque<Msg>>,
}

impl Shared {
    fn pop_inbox(&self) -> Option<Msg> {
        self.inbox.lock().pop_front()
    }
}

struct Worker<L> {
    url: AStr,
    config: JoinConfig,
    connector: TungConnector,
    rx: mpsc::Receiver<()>,
    shared: Arc<Shared>,
    listener: L,
}

impl<L: Listener> Worker<L> {

    #[trace_result]
    async fn run(mut self) {

        let r = self.try_run().await;

        match r {
            Ok(_v) => {
                self.listener.on_closed(proto::Status::default());
                debug!("finished Ok");
            },
            Err(e) => {
                let err = trace_error!("finished error", e);
                error!("{}", fmt_error!(err));
                let status: proto::Status = err.into();
                self.listener.on_closed(status);
            },
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
            self.try_handle_msgs(session).await.with_context(||"try_handle_msgs failed")?;
            
            let r = self.recv_packet(&mut session.conn, &mut rraw).await.with_context(||"recv_packet falied")?;

            let Some(packet) = r else {
                continue;
            };

        }

        // Ok(())
    }

    async fn try_handle_msgs(&mut self, session: &mut Session) -> Result<()> {
        while let Some(msg) = self.shared.pop_inbox() {

        }
        Ok(())
    }

    #[trace_result]
    #[instrument(skip(self, session))]
    async fn phase_reconnect(&mut self, session: &mut Session) -> Result<()> {
        let (conn, raw) = timeout(
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

        Ok(session)

        
        // // 把执行 open_session 的时间也考虑到 total_connect_timeout 里
        // let start_at = Instant::now();

        // while start_at.elapsed() < self.config.connection.total_connect_timeout() {

        //     let r = self.connect_to(start_at).await
        //         .with_context(||"connect_to failed");

        //     let mut stream = match r {
        //         Ok(v) => v,
        //         Err(e) => {
        //             warn!("{:?}", e);
        //             continue;
        //         },
        //     };

        //     let r = self.open_session(&mut stream).await;

        //     match r {
        //         Ok(_v) => {},
        //         Err(e) => {
        //             warn!("{:?}", e);
        //             continue;
        //         },
        //     };

        //     return Ok(stream)
        // }

        // Err(reach_total_connect_timeout(&self.config.connection))
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

        // Err(anyhow!("dropped"))
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

        // Err(anyhow!("dropped"))
    }

    #[trace_result]
    async fn reconnect_session<'a>(&mut self, conn: &mut Conn) -> Result<PacketRaw> {
        Err(anyhow!("Not implement"))
    }

    #[trace_result]
    async fn connect_loop(&mut self) -> Result<Conn> {

        debug!("connecting to [{}]...", self.url);

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

        // Err(anyhow!("dropped"))
    }

    // async fn open_session<'a>(&mut self, conn: &mut Conn, rraw: &'a mut RecvRaw) -> Result<(OpenOutput, proto::PacketRef<'a>)> {
    //     self.open_session_send(conn).await.with_context(||"open_session_send failed")?;
    //     self.open_session_recv(conn, rraw).await.with_context(||"open_session_recv failed")
    // }

    // async fn open_session_recv<'a>(&mut self, conn: &mut Conn, rraw: &'a mut RecvRaw) -> Result<(OpenOutput, proto::PacketRef<'a>)> {

    //     loop {
    //         let r = self.recv_packet(conn, rraw)
    //             .await.with_context(||"rev_packet failed")?;

    //         let Some(packet) = r else  {
    //             continue;
    //         };

    //         debug!("recv open response {packet:?}");

    //         let output = OpenOutput {
    //             packet: packet.to_meta(), 
    //             body: packet.body.as_ref().map(|x|x.to_string()),
    //             // body: packet.body.map(|x|x.into_owned()),
    //         };

    //         return Ok((output, packet))
    //     }

    //     // let mut rraw = RecvRaw::default();

    //     // let packet = loop {
    //     //     let r = self.recv_packet(conn, &mut rraw)
    //     //         .await.with_context(||"rev_packet failed")?;
    //     //     if let Some(packet) = r {
    //     //         break packet;
    //     //     }
    //     // };


    //     // debug!("recv open response {packet:?}");

    //     // let output = OpenOutput {
    //     //     packet: packet.to_meta(), 
    //     //     body: packet.body.as_ref().map(|x|x.to_string()),
    //     //     // body: packet.body.map(|x|x.into_owned()),
    //     // };

    //     // Ok((output, packet))

    // }

    #[trace_result]
    async fn open_session_send(&mut self, conn: &mut Conn) -> Result<()> {
        use crate::proto::IntoIterSerialize;

        let req = proto::OpenSessionRequestSer {
            user_id: &self.config.user_id,
            room_id: &self.config.room_id,
            user_ext: self.config.advance.user_ext.as_deref(),
            user_tree: self.config.advance.user_tree
                .as_ref()
                .map(|x|
                    x.iter().map(|y|proto::UpdateTreeRequestSer::from(y))
                    .into_iter_ser()
                ),
        };

        let packet = proto::PacketSer {
            typ: Some(proto::PacketType::Request.as_num()),
            sn: None, // Open 请求可以没有 sn
            body: Some(req.into_body()),
            ack: None, // 
        };

        conn.send_packet(&packet).await
            .with_context(||"send_packet failed")?;

        Ok(())
    }


    #[trace_result]
    async fn recv_packet<'a>(&mut self, conn: &mut Conn, rraw: &'a mut RecvRaw) -> Result<Option<proto::PacketRef<'a>>> {
        loop {
            let r = self.recv_raw(conn).await?;

            let Some(raw) = r else {
                continue;
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
            debug!("recv raw [{:?}]", r);
            
            let Some(raw) = r else {
                continue;
            };

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

// struct OpenOutput {
//     packet: proto::PacketMeta,
//     body: Option<PacketRaw>,
// }

// impl OpenOutput {
//     fn into_rsp(self) -> Result<proto::OpenSessionResponse> {

//         if self.packet.typ() != Some(proto::PacketType::Response) {
//             bail!("invalid packet type {:?}", self.packet.typ())
//         }

//         let body = self.body.as_ref().with_context(||"empty body")?;

//         let mut server_rsp: proto::ServerResponse = serde_json::from_str(body)?;

//         if let Some(status) = server_rsp.status.take() {
//             return status.with_context(||"open response status")
//         }

//         let rsp_type = server_rsp.typ.with_context(||"invalid response type")?;
//         match rsp_type {
//             proto::response::ResponseType::Open(rsp) => {
//                 Ok(rsp)
//                 // Ok(Session {
//                 //     session_id: rsp.session_id,
//                 //     conn_id: rsp.conn_id,
//                 //     xfer: Xfer::new(),
//                 // })
//             },
//             _ => {
//                 bail!("unexpect response type")
//             }
//         }
//     }
// }

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


#[derive(Debug, Default)]
struct RecvRaw {
    raw: PacketRaw,
}

struct Conn {
    stream: TungStream,
}

impl Conn {
    pub async fn send_packet<B>(&mut self, packet: &proto::PacketSer<B>) -> Result<()> 
    where
        B: serde::Serialize,
    {
        let json = serde_json::to_string(&packet)?;
        debug!("send raw [{}]", json);
        self.stream.send_text(json).await?;
        Ok(())
    }

    // async fn recv_packet<'a>(&'a mut self) -> Result<proto::PacketRef<'a>> {
    //     loop {
    //         self.stream.wait_next_recv().await.with_context(||"wait_next_recv failed")?;
            
    //         let r = self.stream.recv_next_packet().await.with_context(||"wait_next_recv failed")?;
    //         debug!("recv raw [{:?}]", r);
            
    //         let Some(raw) = r else {
    //             continue;
    //         };

    //         self.raw = raw.as_str().into();

    //         let packet: proto::PacketRef = serde_json::from_str(&self.raw.as_str()).with_context(||"parse packet failed")?;

    //         return Ok(packet)

    //     }
    // }
}

type PacketRaw = tokio_tungstenite::tungstenite::Utf8Bytes;

// async fn connect_to(start_at: Instant, url: &str, cfg: &ConnectionConfig) -> Result<TungStream> {
    
//     while start_at.elapsed() < cfg.total_connect_timeout() {
        
//         debug!("connecting to [{}]...", url);
//         let r = timeout(cfg.connect_timeout(), TungStream::connect(url)).await;

//         match r {
//             Ok(Ok(v)) => {
//                 debug!("connected to [{}]", url);
//                 return Ok(v)
//             },
//             Ok(Err(e)) => {
//                 warn!("connect failed, url [{}], error [{:?}]", url, e);
//             }
//             Err(_e) => {
//                 warn!("connect timeout [{:?}], url [{}]", cfg.connect_timeout(), url);
//             },
//         }

//         debug!("will retry in [{:?}]", cfg.retry_interval());
//         async_rt::sleep(cfg.retry_interval()).await;
//     }

//     Err(reach_total_connect_timeout(cfg))
// }

// #[trace_result]
// async fn connect_loop(connector: &TungConnector, url: &str, cfg: &ConnectionConfig, rx: &mut mpsc::Receiver<()>) -> Result<Conn> {
    
//     loop {
        
//         debug!("connecting to [{}]...", url);

//         let r = tokio::select! {
//             r = rx.recv() => {
//                 check_guard(r)?;
//                 continue;
//             }

//             r = timeout(cfg.connect_timeout(), connector.connect(url)) => {
//                 r
//             }
//         };

//         // let r = timeout(cfg.connect_timeout(), connector.connect(url)).await;

//         match r {
//             Ok(Ok(stream)) => {
//                 debug!("connected to [{}]", url);
//                 return Ok(Conn {
//                     stream,
//                 })
//             },
//             Ok(Err(e)) => {
//                 warn!("connect failed, url [{}], error [{:?}]", url, e);
//             }
//             Err(_e) => {
//                 warn!("connect timeout [{:?}], url [{}]", cfg.connect_timeout(), url);
//             },
//         }

//         // let r = connector.connect(url).await;

//         // match r {
//         //     Ok(v) => {
//         //         debug!("connected to [{}]", url);
//         //         return Ok(v)
//         //     },
//         //     Err(e) => {
//         //         warn!("connect failed, url [{}], error [{:?}]", url, e);
//         //     }
//         // }

//         debug!("will retry in [{:?}]", cfg.retry_interval());
//         async_rt::sleep(cfg.retry_interval()).await;
//     }
// }



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



// pub struct ResponseRx(oneshot::Receiver<Result<BizResponse>>);

// impl ResponseRx {
//     pub async fn into_recv<O>(self) -> Result<O> 
//     where
//         O: TryFrom<BizResponse>,
//         O::Error: Into<anyhow::Error>,
//     {
//         let response = self.0.await
//             .with_context(||"request cancelled")?
//             .with_context(||"handle request failed")?;

//         let response = O::try_from(response).map_err(|e|e.into())?;

//         Ok(response)
//     }
// }

// #[derive(Debug)]
// pub enum RecvMessage {
//     RoomNotice(RoomNotice),
//     ClosedStatus(PbStatus),
//     ClosedNotice(PbClosedNotice),
//     SyncLost(PbSyncLost),
//     RoomChanged(PbBizRoomsChanged),
// }

// impl std::fmt::Display for RecvMessage {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::RoomNotice(notice) => {
//                 write!(f, "RoomNotice: id {:?}, seq {}, len {}, binary [{}]", notice.room_id, notice.seq, notice.payload.len(), notice.payload.hex_dump())?;
//                 if let Ok(s) = std::str::from_utf8(&notice.payload) {
//                     write!(f, ", TEXT: [{s}]")?;
//                 } 
//             },
//             RecvMessage::ClosedStatus(_v) => {
//                 std::fmt::Debug::fmt(&self, f)?;
//             },
//             RecvMessage::ClosedNotice(_v) => {
//                 std::fmt::Debug::fmt(&self, f)?;
//             },
//             RecvMessage::SyncLost(_v) => {
//                 std::fmt::Debug::fmt(&self, f)?;
//             },
//             RecvMessage::RoomChanged(_v) => {
//                 std::fmt::Debug::fmt(&self, f)?;
//             },
//         }
//         Ok(())
//     }
// }

// pub struct MsgReceiver {
//     rx: mpsc::Receiver<RecvMessage>,
// }

// impl MsgReceiver {
//     pub async fn recv(&mut self) -> Option<RecvMessage> {
//         self.rx.recv().await
//     }

//     pub async fn recv_timeout(&mut self, millis: u64) -> Result<RecvMessage> {
//         tokio::time::timeout(Duration::from_millis(millis), self.rx.recv())
//         .await.with_context(||"recv timeout")?
//         .with_context(||"recv closed")
//     }

//     pub fn try_recv(&mut self) -> Result<Option<RecvMessage>> {
//         let r = self.rx.try_recv();
//         match r {
//             Ok(msg) => Ok(Some(msg)),
//             Err(mpsc::error::TryRecvError::Empty) => Ok(None),
//             Err(mpsc::error::TryRecvError::Disconnected) => bail!("try_recv but closed")
//         }
//     }
// }


// type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;



struct Entity {
    url: AStr,
    scheme: SchemeClient,
    // ws: Option<WsStream>,
    ws: Option<TungStream>,
    msg_id: NextId,
    // notice_tx: Option<mpsc::Sender<RecvMessage>>,
    // inflights: HashMap<i64, InflightRequest>,
}

impl Entity {
    // async fn try_wait_next(&mut self) -> Next {
    //     if let Some(ws) = self.ws.as_mut() {
    //         let msg = ws.next().await
    //             .with_context(||"recv message but got nothing")?
    //             .with_context(||"recv message failed")?;

    //         match msg {
    //             WsMessage::Binary(v) => {
    //                 return Ok(v)
    //             },
    //             _ => return Err(anyhow!("expect binary message but got [{msg:?}]")),
    //         }
    //     } else {
    //         loop {
    //             std::future::pending::<()>().await;
    //         }
    //     }
    // }

    async fn try_wait_next(&mut self) -> Next {
        if let Some(ws) = self.ws.as_mut() {
            let r= ws.wait_next_recv().await;
            return r
        } else {
            loop {
                std::future::pending::<()>().await;
            }
        }
    }

    async fn try_handle_next(&mut self, next: Next) -> Result<()> {
        next?;
        let recv_bytes = match self.ws.as_mut() {
            Some(ws) => {
                let r = ws.recv_next_packet().await?;
                match r {
                    Some(v) => v,
                    None => return Ok(()),
                }
            },
            None => return Ok(()),
        };
        
        trace!("recv bytes [{:?}], [{}]", recv_bytes.len(), recv_bytes.as_str());

        // let mut server_msg: ServerMessage = recv_bytes.try_to().with_context(||"parse server msg failed")?;
        
        // trace!("recv server message [{:?}]", server_msg);

        // let msg_type = get_field!(server_msg.msg_type)?; 
        // match msg_type {
        //     ServerMsgType::RoomNotice(notice) => {
        //         if let Some(tx) = self.notice_tx.as_ref() {
        //             let r = tx.send(RecvMessage::RoomNotice(notice)).await;
        //             if r.is_err() {
        //                 debug!("room notice but receiver closed");
        //                 self.notice_tx = None;
        //             }
        //         }
        //     },
        //     ServerMsgType::ClosedStatus(status) => {
        //         if let Some(tx) = self.notice_tx.as_ref() {
        //             let r = tx.send(RecvMessage::ClosedStatus(status)).await;
        //             if r.is_err() {
        //                 debug!("closed status but receiver closed");
        //                 self.notice_tx = None;
        //             }
        //         }
        //     },
        //     ServerMsgType::Response(response) => {
        //         self.handle_response(response).await?;
        //     },
        //     ServerMsgType::ClosedNotice(notice) => {
        //         if let Some(tx) = self.notice_tx.as_ref() {
        //             let r = tx.send(RecvMessage::ClosedNotice(notice)).await;
        //             if r.is_err() {
        //                 debug!("notice receiver closed");
        //                 self.notice_tx = None;
        //             }
        //         }
        //     },
        //     ServerMsgType::SyncLost(v) => {
        //         if let Some(tx) = self.notice_tx.as_ref() {
        //             let r = tx.send(RecvMessage::SyncLost(v)).await;
        //             if r.is_err() {
        //                 debug!("notice receiver closed");
        //                 self.notice_tx = None;
        //             }
        //         }
        //     },
        //     ServerMsgType::BizRoomsChanged(v) => {
        //         if let Some(tx) = self.notice_tx.as_ref() {
        //             let r = tx.send(RecvMessage::RoomChanged(v)).await;
        //             if r.is_err() {
        //                 debug!("notice receiver closed");
        //                 self.notice_tx = None;
        //             }
        //         }
        //     }

        //     // _ => {
        //     //     bail!("unexpected message type [{msg_type:?}]")
        //     // }
        // }
        Ok(())
    }

    // async fn handle_response(&mut self, mut response: sdk::Response) -> Result<()> {
    //     // if let Some(status) = extract_error_status!(response) {
    //     if let Err(status) = response.status.take().unwrap_or_default().into_result() {
    //         match self.inflights.remove(&response.msg_id) {
    //             Some(inflight) => {
    //                 if let Some(tx) = inflight.tx {
    //                     let _r = tx.send(Err(status.into()));
    //                 }
    //             },
    //             None => {
    //                 warn!("recv response but NOT found request, msg_id [{}]", response.msg_id);
    //             },
    //         }
    //         return Ok(())
    //     }

    //     let response_type = get_field!(response.response_type)?;
    //     match response_type {
    //         ResponseType::BizResponse(biz_rsp) => {
    //             match self.inflights.remove(&response.msg_id) {
    //                 Some(inflight) => {
    //                     if let Some(tx) = inflight.tx {
    //                         let _r = tx.send(Ok(biz_rsp));
    //                     }

    //                     if inflight.req_close {
    //                         self.ws.take();
    //                     }
    //                 },
    //                 None => {
    //                     warn!("recv response but NOT request, msg_id [{}]", response.msg_id);
    //                 },
    //             }
    //         },
    //         ResponseType::FirstOpenResponse(_rsp) => {
    //             warn!("recv unexpect FirstOpenResponse");
    //         },
    //         ResponseType::OpenSubSessionResponse(_rsp) => {
    //             warn!("recv unexpect OpenSubSessionResponse");
    //         }
    //         ResponseType::OpenSessionResponse(_rsp) => {
    //             warn!("recv unexpect OpenSessionResponse");
    //         },
    //         ResponseType::ReconnectResponse(_) => {
    //             warn!("recv unexpect ReconnectResponse");
    //         },
    //         ResponseType::HeartbeatResponse(_) => {
    //             warn!("recv unexpect HeartbeatResponse");
    //         }
    //     }
    //     Ok(())
    // }

    async fn try_handle_msg(&mut self, msg: Msg) -> Result<()> {
        match msg {
            Msg::Request(req) => {
                // let msg_id = self.msg_id.next();
                // let r = self.send_biz_request(msg_id, req.request).await;

                // match r {
                //     Ok(_r) => {
                //         self.inflights.insert(msg_id, InflightRequest {
                //             tx: Some(req.tx),
                //             req_close: req.req_close,
                //         });
                //     },
                //     Err(e) => {
                //         let _r = req.tx.send(Err(e));
                //     },
                // }

            },
        }
        Ok(())
    }

    // async fn send_biz_request(&mut self, msg_id: i64, req: BizRequest) -> Result<()> {
    //     match self.ws.as_mut() {
    //         Some(ws) => {
    //             let client_msg = ClientMessage {
    //                 msg_id,
    //                 msg_type: Some(client_message::MsgType::BizRequest(req)),
    //             };
    //             ws.send_message(&client_msg, "send_biz_request").await
    //             // send_pb_msg(ws, &client_msg).await
    //         },
    //         None => {
    //             Err(anyhow!("Not connected"))
    //         },
    //     }
    // }

    // async fn try_connect(&mut self) -> Result<TungStream> {
    //     if self.ws.is_some() {
    //         return Err(anyhow!("already connected"))
    //     }
    //     debug!("try connecting to [{}]..", self.url);

    //     let socket = TungStream::connect(&self.url).await
    //         .with_context(||"connect failed")?;

    //     debug!("connected to [{}]", self.url);
    //     Ok(socket)
    // }

    // fn make_receiver(&mut self) -> Result<MsgReceiver> {
    //     if let Some(tx) = self.notice_tx.as_mut() {
    //         if !tx.is_closed() {
    //             bail!("already started receive notice")
    //         }
    //     }
    //     let (tx, rx) = mpsc::channel(32);
    //     self.notice_tx = Some(tx);
    //     Ok(MsgReceiver { rx })
    // }

}

#[derive(Debug, Default)]
struct NextId(i64);
impl NextId {
    pub fn next(&mut self) -> i64 {
        self.0 += 1;
        self.0
    }
}



// async fn send_pb_msg<M: PbMsg>(ws: &mut WsStream, msg: &M) -> Result<()> {
//     let data = msg.encode_to_vec();
//     ws.send(WsMessage::Binary(data)).await?;
//     Ok(())
// }

// struct InflightRequest {
//     tx: Option<oneshot::Sender<Result<BizResponse>>>,
//     req_close: bool,
// }


// #[async_trait::async_trait]
// impl AsyncHandler<OpenInput> for Entity {
//     type Response = Result<OpenOutput>; //: MessageResponse<Self, M>;

//     async fn handle(&mut self, cmd: OpenInput) -> Self::Response {

//         let mut socket = self.try_connect().await?;

//         let heartbeat_interval = cmd.heartbeat.unwrap_or(0) as i32;

//         // self.msg_id = 1;
//         let first_req = ClientMessage { 
//             msg_id: self.msg_id.next(), 
//             msg_type: Some(client_message::MsgType::FirstOpenRequest(FirstOpenRequest { 
//                 magic: MAGIC, 
//                 proto_ver: PROTO_VER_1, 
//                 open_sub_session_request: Some(OpenSubSessionRequest {
//                     biz_requests: cmd.biz_requests,
//                 }), 
//                 proposal: if heartbeat_interval == 0 {
//                     None
//                 } else {
//                     Some(ClientProposal {
//                         heartbeat_interval,
//                     })
//                 }, 
//             })),
//         };

//         // debug!("first request: {first_req:?}");


//         socket.send_message(&first_req, "first req").await.with_context(||"send first msg failed")?;

//         let mut recv_msg: ServerMessage = socket.read_raw_packet().await
//         .with_context(||"recv first packet failed")?
//         .try_to().with_context(||"decode first packet failed")?;

//         // debug!("recv first msg [{:?}]", recv_msg);

//         let mut first_rsp = extract_first_open_response(&mut recv_msg)
//             .with_context(|| format!("extract_open_response failed, [{:?}]", recv_msg))?;

//         let open_rsp = get_field!(first_rsp.open_sub_session_response)?;
        
//         self.ws = Some(socket);

//         Ok(OpenOutput { 
//             msg_receiver: self.make_receiver()?,
//             session_id: first_rsp.session_id,
//             biz_responses: open_rsp.biz_responses,
//         })
//     }
// }

// struct OpenInput {
//     biz_requests: Vec<BizRequest>,
//     heartbeat: Option<i64>,
// }

// pub struct OpenOutput {
//     pub session_id: String,
//     pub biz_responses: Vec<BizResponse>,
//     pub msg_receiver: MsgReceiver,
// }

// // const PROTO_VER: i32 = 1;

// #[async_trait::async_trait]
// impl AsyncHandler<ReconnectInput> for Entity {
//     type Response = Result<ReconnectOutput>; //: MessageResponse<Self, M>;

//     async fn handle(&mut self, cmd: ReconnectInput) -> Self::Response {

//         let mut socket = self.try_connect().await?;

//         // self.msg_id = 1;

//         let first_req = ClientMessage { 
//             msg_id: self.msg_id.next(), 
//             msg_type: Some(client_message::MsgType::ReconnectRequest(cmd.req)),
//         };

//         // let first_req = ClientFirstMessage { 
//         //     msg_id: self.msg_id.next(), 
//         //     msg_type: Some(ClientFirstMsgType::ReconnectRequest(cmd.req)),
//         //     proto_ver: PROTO_VER,
//         //     proposal: Default::default(),
//         // };

//         debug!("first request: {first_req:?}");


//         socket.send_message(&first_req, "first req").await.with_context(||"send first msg failed")?;

//         let recv_msg: ServerMessage = socket.read_raw_packet().await
//         .with_context(||"recv first packet failed")?
//         .try_to().with_context(||"decode first packet failed")?;

//         debug!("recv first msg [{:?}]", recv_msg);

//         let rsp = extract_reconnect_response(recv_msg)
//             .with_context(||"extract_reconnect_response failed")?;
        
//         self.ws = Some(socket);

//         Ok(ReconnectOutput { 
//             msg_receiver: self.make_receiver()?,
//             rsp,
//         })
//     }
// }

// struct ReconnectInput {
//     req: ReconnectRequest,
// }

// pub struct ReconnectOutput {
//     pub rsp: ReconnectResponse,
//     pub msg_receiver: MsgReceiver,
// }


#[async_trait::async_trait]
impl AsyncHandler<CloseCmd> for Entity {
    type Response = Result<()>; //: MessageResponse<Self, M>;

    async fn handle(&mut self, _cmd: CloseCmd) -> Self::Response {
        self.ws.take();
        Ok(())
    }
}

struct CloseCmd {}

// #[async_trait::async_trait]
// impl AsyncHandler<NoticeRecverCmd> for Entity {
//     type Response = Result<NoticeRecverResponse>; //: MessageResponse<Self, M>;

//     async fn handle(&mut self, _cmd: NoticeRecverCmd) -> Self::Response {
//         if let Some(tx) = self.notice_tx.as_mut() {
//             if !tx.is_closed() {
//                 bail!("already started receive notice")
//             }
//         }
//         let (tx, rx) = mpsc::channel(32);
//         self.notice_tx = Some(tx);
//         Ok(NoticeRecverResponse(NoticeReceiver { rx }))
//     }
// }

// struct NoticeRecverCmd {}
// struct NoticeRecverResponse(NoticeReceiver);

struct Request {
    // request: BizRequest,
    // tx: oneshot::Sender<Result<BizResponse>>,
    // req_close: bool,
}

enum Msg {
    Request(Request),
}

impl ActorHandler for Entity {
    type Next = Next;

    type Msg = Msg;

    type Result = ();

    fn wait_next(&mut self) -> impl Future<Output = Self::Next> + Send {
        self.try_wait_next()
    }

    fn handle_next(&mut self, next: Self::Next) -> impl Future<Output = ActionRes> + Send {
        async {
            self.try_handle_next(next).await?;
            Ok(Action::None)
        }
    }

    fn handle_msg(&mut self, msg: Self::Msg) -> impl Future<Output = ActionRes> + Send {
        async {
            self.try_handle_msg(msg).await?;
            Ok(Action::None)
        }
    }
    
    fn into_result(self) -> Self::Result {
        ()
    }
}

// type Next = Result<Vec<u8>>;
type Next = Result<()>;




// fn extract_open_response(mut msg: ServerMessage) -> Result<OpenSessionResponse> {
        
//     let msg_type = extract_opt!(msg.msg_type)?;

//     let mut rsp = match msg_type {
//         ServerMsgType::Response(v) => v,
//         _ => bail!("expect first response but [{msg_type:?}]")
//     };

//     extract_opt!(rsp.status)?.into_result()?;
    
//     let rsp_type = extract_opt!(rsp.response_type)?;

//     let open_rsp = match rsp_type {
//         ResponseType::OpenSessionResponse(v) => v,
//         _ => bail!("expect open response but [{rsp_type:?}]")
//     };

//     Ok(open_rsp)
// }




#[derive(Debug, Clone)]
enum SchemeClient {
    Ws,
}


// #[tokio::test]
// async fn test() {
//     test_wss().await.unwrap();
// }

// async fn test_wss() -> Result<()> {
//     rustls::crypto::ring::default_provider().install_default()
//         .map_err(|e| anyhow::anyhow!("failed to install default crypto [{e:?}]"))?;

//     let url = String::from("wss://127.0.0.1:11443/ws");

//     let verifier = crate::kit::tls_utils::verifier::CustomCertVerifier::new_dummy();

//     let client_crypto = {

//         rustls::ClientConfig::builder()
//         .dangerous()
//         .with_custom_certificate_verifier(verifier)
//         .with_no_client_auth()
//     };

//     let client_crypto = std::sync::Arc::new(client_crypto);


//     // let uri: Uri = url.parse().unwrap();
//     // let host = uri.host().unwrap_or("");
//     // let addr = format!("{}:{}", host, uri.port_u16().unwrap_or(443));
//     // let tcp = tokio::net::TcpStream::connect(&addr).await?;

//     // // let connector = tokio_rustls::TlsConnector::from(client_crypto.clone());
//     // // let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
//     // //     .map_err(|_| anyhow::anyhow!("invalid DNS name [{}]", host))?;
//     // // let tls_stream = connector.connect(server_name, tcp).await?;
//     // // let stream = tokio_tungstenite::client_async(url.as_str(), tls_stream).await?;
    
//     // let stream = tokio_tungstenite::client_async_tls_with_config(url.as_str(), tcp, None, Some(tokio_tungstenite::Connector::Rustls(client_crypto.clone()))).await?;

//     let stream = tokio_tungstenite::connect_async_tls_with_config(url.as_str(), None, false, Some(tokio_tungstenite::Connector::Rustls(client_crypto.clone()))).await?;
//     println!("connected to url [{}]", url);

//     Ok(())
// }

