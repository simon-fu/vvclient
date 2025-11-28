
use bytes::Bytes;
use futures::{SinkExt as _, StreamExt};
// use mp_common_rs::prost;
use tokio::net::TcpStream;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream, connect_async, connect_async_tls_with_config, tungstenite::{Message as WsMessage, Utf8Bytes, protocol::WebSocketConfig}};
use anyhow::{Context as _, Result};
use trace_error::anyhow::trace_result;
use tracing::debug;
use crate::kit::tls_utils;
// use super::super::LiveConnection;


#[derive(Debug, Clone, Default)]
pub struct TungConfig {
    /// 忽略服务器的证书  
    pub ignore_tls_cert: bool,

    /// 禁止 nagle 算法
    pub disable_nagle: bool,
}

#[derive(Clone, Default)]
pub struct TungConnector {
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    connector: Option<Connector>,
}

impl TungConnector {
    pub fn new(ignore_server_cert: bool) -> Self {
        let connector = if ignore_server_cert {
            let verifier = tls_utils::verifier::CustomCertVerifier::new_dummy();

            let client_crypto = {

                rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth()
            };

            let client_crypto = std::sync::Arc::new(client_crypto);
            Some(Connector::Rustls(client_crypto))
        } else {
            None
        };

        Self {
            connector,
            ..Default::default()
        }
    }

    #[trace_result]
    pub async fn connect(&self, url: &str) -> Result<TungStream> {
        let (socket, _rsp) = connect_async_tls_with_config(
            url, 
            self.config.clone(), 
            self.disable_nagle, 
            self.connector.clone(),
        ).await.with_context(||format!("failed to connect to [{}]", url))?;

        Ok(TungStream {
            socket,
            recv_packet: Default::default(),
            recv_ping: Default::default(),
            is_closed: Default::default(),
        })
    }
}

pub struct TungStream {
    socket: WsStream,
    recv_packet: Option<Packet>,
    recv_ping: Option<Bytes>,
    // close_frame: Option<Option<CloseFrame<'static>>>,
    is_closed: bool
}

impl TungStream {
    pub async fn connect(url: &str) -> Result<Self> {
        let (socket, _rsp) = connect_async(url).await
            .with_context(||format!("failed to connect to [{}]", url))?;
        Ok(Self {
            socket,
            recv_packet: Default::default(),
            recv_ping: Default::default(),
            is_closed: Default::default(),
        })
    }

    pub async fn send_text(&mut self, text: String) -> Result<()> {
        self.socket.send(WsMessage::Text(text.into())).await?;
        Ok(())
    }

    pub async fn wait_next_recv(&mut self)-> Result<()> {
        if self.recv_packet.is_some() {
            return Ok(())
        }

        loop {
            
            let r = self.socket.next().await;

            let message = match r {
                Some(r) => r.with_context(||"failed to receive message")?,
                None => {
                    debug!("websocket recv None, maybe cloesed");
                    self.is_closed = true;
                    break;
                },
            };
            // debug!("recv ws frame {message:?}");

            match message {
                WsMessage::Binary(msg) => {
                    debug!("recv message but got binary [{}]", msg.len());
                    // break;
                },
                WsMessage::Close(frame) => {
                    debug!("recv close-frame {frame:?}");
                    self.is_closed = true;
                    break;
                }
                WsMessage::Ping(data) => {
                    self.recv_ping = Some(data);
                }
                WsMessage::Pong(data) => {
                    debug!("recv message but got pong, len [{}]", data.len());
                }
                WsMessage::Text(text) => {
                    // debug!("recv message but got text [{text}]");
                    self.recv_packet = Some(text.into());
                    break;
                }
                WsMessage::Frame(v) => unreachable!("{v:?}"),
            }
        }

        Ok(())
    }
    
    pub async fn recv_next_packet(&mut self) -> Result<Option<Packet>> {
        if let Some(data) = self.recv_ping.take() {
            self.socket.send(WsMessage::Pong(data)).await.with_context(||"send pong failed")?;
        }
        Ok(self.recv_packet.take())
    }

    // pub async fn try_recv_packet(&mut self) -> Result<Packet> {
    //     loop {
    //         self.wait_next_recv().await.with_context(||"wait_next_recv failed")?;
    //         let r = self.recv_next_packet().await.with_context(||"recv_next_packet failed")?;
    //         if let Some(packet) = r {
    //             return Ok(packet)
    //         }
    //     }
    // }
}

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Packet = Utf8Bytes;

