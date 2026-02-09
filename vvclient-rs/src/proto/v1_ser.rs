

use std::collections::HashMap;

use crate::proto::{fmt_writer::JsonDisplay, mediasoup};

use super::UpdateTreeRequest;
// use super::mediasoup;

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct PacketSer<B> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<i32>,  // see PacketType

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<i64>,  // seq number

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<B>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<i64>,  // ack sn
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ClientRequestSer<T> {
    pub typ: T,
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum OpenTypeSer<T> {
    Open(T),

    // Reconn(super::ReconnectRequest),

    // Close(super::CloseSessionRequest),

    // CreateX(super::CreateWebrtcTransportRequest),

    // ConnX(super::ConnectWebrtcTransportRequest),

    // Pub(super::PublishRequest),

    // UPub(super::UnPublishRequest),

    // Sub(super::SubscribeRequest),

    // USub(super::UnSubscribeRequest),

    // Mute(super::MuteRequest),

    // Layer(super::LayerRequest),

    // End(super::EndRequest),

    // UpExt(super::UpdateExtRequest),

    // UpUTree(super::UpdateTreeRequest),

    // UpRTree(super::UpdateTreeRequest),

    // Chat(super::ChatRequest),

    // Ack(super::PushAck),
}



#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct OpenSessionRequestSer<'a, UTREE, DEVICE> 
// where 
//     // UTREE: Iterator<Item = UpdateTreeRequestRef<'a>> + 'a,
//     UTREE: serde::Serialize,
{
    pub user_id: &'a str,

    pub room_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfoSer<'a, DEVICE>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ext: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_tree: Option< UTREE >,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<bool>,

    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub create_x_requests: Option<XREQS>,  
}

impl<'a, UTREE, DEVICE> OpenSessionRequestSer<'a, UTREE, DEVICE> {
    fn into_msg(self) -> ClientRequestSer<OpenTypeSer<Self>> {
        ClientRequestSer {
            typ: OpenTypeSer::Open(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<OpenTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ClientInfoSer<'a, DEVICE=HashMap<&'a str, &'a str>> {
    /// 客户端平台类型，如 Android / iOS / Windows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkInfoSer<'a>>,

    /// 平台相关的设备信息，使用 key-value 承载
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<DEVICE>, // HashMap<String, String> 或者 HashMap<&str, &str>
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct SdkInfoSer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<&'a str>,
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum ReconnTypeSer<T> {
    Reconn(T),
}



#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ReconnectRequestSer<'a> 
{
    pub session_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<&'a str>,

    /// 尝试重连的次数，从1开始，依次递增。
    /// 比如 第一次重连是1，第二次是2，不管连接成功与否，每次尝试连接都递增
    pub try_seq: i64,

    /// 上一次重连成功的 try_seq 值，初始值为 0
    pub last_success_seq: i64,

    /// 标志， 固定为 20250901
    pub magic: i32, 
}

impl<'a> ReconnectRequestSer<'a> {
    fn into_msg(self) -> ClientRequestSer<ReconnTypeSer<Self>> {
        ClientRequestSer {
            typ: ReconnTypeSer::Reconn(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<ReconnTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct UpdateTreeRequestSer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

impl<'a> From<&'a UpdateTreeRequest> for UpdateTreeRequestSer<'a> {
    fn from(src: &'a UpdateTreeRequest) -> Self {
        Self {
            path: src.path.as_deref(),
            value: src.value.as_deref(),
            prune: src.prune,
        }
    }
}

/// 创建 WebRtcTransport 请求参数
#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebrtcTransportRequestSer<'a, DTLS> {

    /// Room Id
    pub room_id: &'a str,

    /// 传输通道方向, 见 Direction  
    pub dir: i32,

    /// 媒体类型, 见 MediaKind
    pub kind: i32,

    /// 客户端的 Dtls Parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtls: Option<DTLS>, // mediasoup::prelude::DtlsParameters
}

impl<'a, DTLS> CreateWebrtcTransportRequestSer<'a, DTLS> {
    pub fn into_msg(self) -> ClientRequestSer<CreateXTypeSer<Self>> {
        ClientRequestSer {
            typ: CreateXTypeSer::CreateX(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<CreateXTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum CreateXTypeSer<T> {
    CreateX(T),
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWebrtcTransportRequestSer<'a, DTLS> {
    /// Transport Id
    pub xid: &'a str,

    /// Client Dtls Parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtls: Option<DTLS>,
}

impl<'a, DTLS> ConnectWebrtcTransportRequestSer<'a, DTLS> {
    pub fn into_msg(self) -> ClientRequestSer<ConnXTypeSer<Self>> {
        ClientRequestSer {
            typ: ConnXTypeSer::ConnX(self)
        }
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum ConnXTypeSer<T> {
    ConnX(T),
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IceCandidateSer<'a> {
    /// Unique identifier that allows ICE to correlate candidates that appear on multiple
    /// transports.
    pub foundation: &'a str,
    /// The assigned priority of the candidate.
    pub priority: u32,
    /// The IP address or hostname of the candidate.
    pub ip: &'a str, // address
    /// The protocol of the candidate.
    pub protocol: mediasoup::prelude::Protocol,
    /// The port for the candidate.
    pub port: u16,
    /// The type of candidate (always `Host`).
    pub r#type: mediasoup::prelude::IceCandidateType,
    /// The type of TCP candidate (always `Passive`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_type: Option<mediasoup::prelude::IceCandidateTcpType>,
}

impl<'a> From<&'a mediasoup::prelude::IceCandidate> for IceCandidateSer<'a> {
    fn from(value: &'a mediasoup::prelude::IceCandidate) -> Self {
        Self {
            foundation: &value.foundation,
            priority: value.priority,
            ip: &value.address,
            protocol: value.protocol,
            port: value.port,
            r#type: value.r#type,
            tcp_type: value.tcp_type,
        }
    }
}


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequestSer<'a, RTP> {
    /// Room Id
    pub room_id: &'a str,
    
    /// 发送通道 Transport Id
    pub xid: &'a str,
    
    pub stream_id: &'a str,
    
    /// 媒体类型, 见 MediaKind
    // pub kind: i32,

    /// 媒体流类型
    pub stype: i32,

    /// Rtp Parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtp: Option<RTP>, // ::core::option::Option<mediasoup::prelude::RtpParameters>,

    /// 音频类型, 见 AudioType
    pub audio_type: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,

}

impl<'a, RTP> PublishRequestSer<'a, RTP> {
    pub fn into_msg(self) -> ClientRequestSer<PublishTypeSer<Self>> {
        ClientRequestSer {
            typ: PublishTypeSer::Pub(self)
        }
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum PublishTypeSer<T> {
    Pub(T),
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnPublishRequestSer<'a> {
    /// Room Id
    pub room_id: &'a str,

    /// 生产者 Id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_id: Option<&'a str>,

    /// 媒体流 Id （和生产者 Id 二选一）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<&'a str>,

}

impl<'a> UnPublishRequestSer<'a> {
    pub fn into_msg(self) -> ClientRequestSer<UnPublishTypeSer<Self>> {
        ClientRequestSer {
            typ: UnPublishTypeSer::UPub(self)
        }
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum UnPublishTypeSer<T> {
    UPub(T),
}



#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MuteRequestSer<'a> {
    
    pub room_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_id: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<&'a str>,

    pub muted: bool,
}


impl<'a> MuteRequestSer<'a> {
    pub fn into_msg(self) -> ClientRequestSer<MuteTypeSer<Self>> {
        ClientRequestSer {
            typ: MuteTypeSer::Mute(self)
        }
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum MuteTypeSer<T> {
    Mute(T),
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequestSer<'a> {
    /// Room Id
    pub room_id: &'a str,

    /// 流id
    pub stream_id: &'a str,

    /// 生产者 Id
    pub producer_id: &'a str,

    /// 接收通道 Transport Id
    pub xid: &'a str,

    /// 空间层/时间层
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_layers: Option<super::PreferredLayers>,
}

impl<'a> SubscribeRequestSer<'a> {
    pub fn into_msg(self) -> ClientRequestSer<SubTypeSer<Self>> {
        ClientRequestSer {
            typ: SubTypeSer::Sub(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<SubTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum SubTypeSer<T> {
    Sub(T),
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeRequestSer<'a> {
    pub room_id: &'a str,

    /// 消费者 Id
    pub consumer_id: &'a str,

}

impl<'a> UnsubscribeRequestSer<'a> {
    pub fn into_msg(self) -> ClientRequestSer<USubTypeSer<Self>> {
        ClientRequestSer {
            typ: USubTypeSer::USub(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<USubTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum USubTypeSer<T> {
    USub(T),
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatchEndRequestSer {

}

impl BatchEndRequestSer {
    pub fn into_msg(self) -> ClientRequestSer<BatchEndTypeSer<Self>> {
        ClientRequestSer {
            typ: BatchEndTypeSer::BEnd(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<BatchEndTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum BatchEndTypeSer<T> {
    BEnd(T),
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatRequestSer {

}

impl HeartbeatRequestSer {
    pub fn into_msg(self) -> ClientRequestSer<HeartbeatTypeSer<Self>> {
        ClientRequestSer {
            typ: HeartbeatTypeSer::HB(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<HeartbeatTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum HeartbeatTypeSer<T> {
    HB(T),
}


#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct CloseSessionRequestSer<'a> {

    /// Room Id
    pub room_id: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<bool>,
}

impl<'a> CloseSessionRequestSer<'a> {
    pub fn into_msg(self) -> ClientRequestSer<CloseSessionTypeSer<Self>> {
        ClientRequestSer {
            typ: CloseSessionTypeSer::Close(self)
        }
    }

    pub fn into_body(self) -> JsonDisplay<ClientRequestSer<CloseSessionTypeSer<Self>>> {
        JsonDisplay(self.into_msg())
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum CloseSessionTypeSer<T> {
    Close(T),
}



pub trait IntoIterSerialize {
    fn into_iter_ser(self) -> impl serde::Serialize;
}

impl<I, O> IntoIterSerialize for I 
where 
    I: Iterator<Item = O> + Clone,
    O: serde::Serialize,
{
    fn into_iter_ser(self) -> impl serde::Serialize {
        IterSer(self)
    }
}


#[derive(Clone, PartialEq, Debug)]
pub struct IterSer<I>(pub I);

impl<I, O> serde::Serialize for IterSer<I> 
where 
    I: Iterator<Item = O> + Clone,
    O: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        use serde::ser::SerializeSeq;

        let iter = self.0.clone();

        // 如果能从迭代器得到 size_hint().1，可传入 Some(len)
        let (lower, upper) = iter.size_hint();
        let mut seq = match upper {
            Some(u) if u == lower => serializer.serialize_seq(Some(u))?,
            Some(u) => serializer.serialize_seq(Some(u))?,
            None => serializer.serialize_seq(None)?,
        };

        for item in iter {
            seq.serialize_element(&item)?;
        }

        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use crate::proto::PacketType;

    use super::*;

    #[test]
    fn test() {
        let user_tree = Some(vec![
            UpdateTreeRequest {
                path: Some("foo/bar".into()), 
                value: Some("value-123".into()),
                prune: None,
            },
            UpdateTreeRequest {
                path: Some("root/child".into()), 
                value: Some("child-xyz".into()),
                prune: Some(true),
            },
        ]);

        let req = OpenSessionRequestSer {
            user_id: "John",
            room_id: "Room1",
            user_ext: Some("ext-abc"),
            user_tree: user_tree
                .as_ref()
                .map(|x|
                    x.iter().map(|y|UpdateTreeRequestSer::from(y))
                    .into_iter_ser()
                ),
            batch: None,
            client_info: Option::<ClientInfoSer<'static>>::None,
            token: None,
            // create_x_requests: Option::<()>::None,
        };

        let packet = PacketSer {
            typ: Some(PacketType::Request.as_num()),
            sn: Some(1), 
            body: Some(req.into_body()),
            ack: None, 
        };

        let json = serde_json::to_string(&packet).unwrap();
        println!("{}", json);
        assert_eq!(
            json,
            r#"{"typ":3,"sn":1,"body":"{\"typ\":{\"Open\":{\"user_id\":\"John\",\"room_id\":\"Room1\",\"user_ext\":\"ext-abc\",\"user_tree\":[{\"path\":\"foo/bar\",\"value\":\"value-123\"},{\"path\":\"root/child\",\"value\":\"child-xyz\",\"prune\":true}]}}}"}"#,
        );

    }
}
