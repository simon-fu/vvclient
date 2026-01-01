
use std::{borrow::Cow, collections::HashMap, fmt};

use anyhow::{Context as _, Result, bail};
use strum_macros::FromRepr;

use crate::kit::astr::AStr;

use super::mediasoup;


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(bound(deserialize = "'de: 'a"))]
pub struct PacketRef<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<i32>,  // see PacketType

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<i64>,  // seq number

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Cow<'a, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<i64>,  // ack sn
}

impl<'a> PacketRef<'a> {
    pub fn from_json(json: &'a str) -> Result<Self> {
        let me = serde_json::from_str(json)?;
        Ok(me)
    }

    pub fn typ(&self) -> Option<PacketType> {
        PacketType::from_num(self.typ.clone()?)
    }

    pub fn ack_json(sn: i64) -> Result<String> {
        let json = serde_json::to_string(&Self {
            ack: Some(sn),
            ..Default::default()
        })?;
        Ok(json)
    }

    pub fn qos0(typ: PacketType, body: &'a str) -> Self {
        Self {
            sn: None,
            typ: Some(typ.as_num()),
            body: Some(Cow::Borrowed(body)),
            ..Default::default()
        }
    }

    pub fn qos0_json(typ: PacketType, body: &'a str) -> Result<String> {
        let json = serde_json::to_string(&Self::qos0(typ, body))?;
        Ok(json)
    }

    pub fn qos1(sn: i64, typ: PacketType, body: &'a str) -> Self {
        Self {
            sn: Some(sn),
            typ: Some(typ.as_num()),
            body: Some(Cow::Borrowed(body)),
            ..Default::default()
        }
    }

    // pub fn p1_json(sn: i64, typ: PacketType, body: &'a str) -> Result<String> {
    //     let json = serde_json::to_string(&Self::p1(sn, typ, body))?;
    //     Ok(json)
    // }

    pub fn sn(&self) -> Option<i64> {
        match self.sn {
            None => None,
            Some(0) => None,
            Some(v) => Some(v)
        }
    }

    pub fn parse_as_client_req(&self) -> Result<ClientRequest> {

        let typ = self.typ();

        match typ {
            Some(PacketType::Request) => {}
            _ => bail!("expect request but typ [{:?}]", self.typ)
        }

        let body = self.body.as_ref().with_context(||format!("expect request body"))?;
        let req : ClientRequest = serde_json::from_str(&body).with_context(||"invalid client req body")?;
        Ok(req)
    }

    pub fn parse_as_server_rsp(&self) -> Result<ServerResponse> {

        // let typ = self.typ();

        // match typ {
        //     Some(PacketType::Response) => {}
        //     _ => return Err(anyhow!("expect response but typ [{:?}]", self.typ))
        // }

        let body = self.body.as_ref().with_context(||format!("expect server response body"))?;
        let typed : ServerResponse = serde_json::from_str(&body).with_context(||"invalid server response body")?;
        Ok(typed)
    }

    pub fn parse_as_server_push(&self) -> Result<ServerPush> {
        let body = self.body.as_ref().with_context(||format!("expect server push body"))?;
        let typed : ServerPush = serde_json::from_str(&body).with_context(||"invalid server push body")?;
        Ok(typed)
    }

    pub fn to_meta(&self) -> PacketMeta {
        PacketMeta {
            typ: self.typ,
            sn: self.sn,
            ack: self.ack,
        }
    }

}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
pub struct PacketMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<i32>,  // see PacketType

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<i64>,  // seq number

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<i64>,  // ack sn
}

impl PacketMeta {
    pub fn typ(&self) -> Option<PacketType> {
        PacketType::from_num(self.typ.clone()?)
    }
}




#[derive( Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
#[repr(i32)]
pub enum PacketType {
    Request = 3,    // 请求
    Response = 4,   // 响应
    Push0 = 5,  // 不需要回 ack
    Push1 = 6,  // 不需要立即回 ack
    Push2 = 7,  // 尽可能快回 ack
}

impl PacketType {
    pub fn from_num(v: i32) -> Option<Self> {
        Self::from_repr(v)
    }

    pub fn as_num(&self) -> i32 {
        *self as i32
    }
}



#[derive( Debug, Clone, Copy)]
pub enum BodyType {
    UserInit = 1,
    UserState = 2,
    UserTree = 3,
    UserReady = 4,
    RoomTree = 5,
    RoomReady = 6,
    Chat = 7,
}

impl BodyType {
    pub fn as_num(&self) -> i32 {
        *self as i32
    }
}


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Status {
    pub code :i32,
    pub reason: String,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Status {{ code: {}, reason: {:?} }}",
            self.code, self.reason
        )
    }
}

impl std::error::Error for Status {}

impl Status {
    pub const INTERNAL_ERROR_CODE: i32 = 1001;
    pub fn with_context<C, F, T>(self, f: F) -> Result<T, anyhow::Error>
    where
        C: fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C 
    {
        Err(anyhow::Error::from(self)).with_context(f)
    }
}

impl<'a> From<&'a anyhow::Error> for Status {
    fn from(value: &'a anyhow::Error) -> Self {
        match value.downcast_ref::<Self>() {
            Some(v) => v.clone(),
            None => {
                Self {
                    code: Self::INTERNAL_ERROR_CODE,
                    reason: format!("internal error: [{value}]"),
                }
            },
        }
    }
}


impl From<anyhow::Error> for Status {
    fn from(value: anyhow::Error) -> Self {
        match value.downcast::<Self>() {
            Ok(v) => v,
            Err(e) => {
                Self {
                    code: Self::INTERNAL_ERROR_CODE,
                    reason: format!("internal error: [{e}]"),
                }
            },
        }
    }
}



pub type ClientRequest = ClientMessage;

pub type ServerResponse = Response;

pub type ServerResponseRef<'a> = ResponseRef<'a>;

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ServerPushRef<'a> {
    typ: ServerPushTypeRef<'a>,
}

impl<'a> ServerPushRef<'a> {
    pub fn closed(v: ClosedNoticeRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::Closed(v) }
    }

    pub fn chat(v: ChatNoticeRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::Chat(v) }
    }

    pub fn user_init(v: IdRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::UInit(v) }
    }

    pub fn user_ready(v: IdRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::UReady(v) }
    }

    pub fn user_full(v: &'a UserState) -> Self {
        Self { typ: ServerPushTypeRef::UFull(v) }
    }

    pub fn user_tree(v: TreeStateRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::UTree(v) }
    }

    pub fn room_tree(v: TreeStateRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::RTree(v) }
    }

    pub fn room_ready(v: IdRef<'a>) -> Self {
        Self { typ: ServerPushTypeRef::RReady(v) }
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub enum ServerPushTypeRef<'a> {
    Closed(ClosedNoticeRef<'a>),
    Chat(ChatNoticeRef<'a>),
    /// User Init
    UInit(IdRef<'a>),
    /// User Ready
    UReady(IdRef<'a>),
    /// User Full
    UFull(&'a UserState), 
    /// User Tree
    UTree(TreeStateRef<'a>),
    /// Room Tree 
    RTree(TreeStateRef<'a>),
    /// Room Ready 
    RReady(IdRef<'a>),
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct IdRef<'a> {
    pub id: &'a str,
}

#[derive(serde::Deserialize, Clone, PartialEq, Debug)]
pub struct Id {
    pub id: String,
}

#[derive(serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ServerPush {
    pub typ: ServerPushType,
}

#[derive(serde::Deserialize, Clone, PartialEq, Debug)]
pub enum ServerPushType {
    Closed(ClosedNotice),
    Chat(ChatNotice),
    /// User Init
    UInit(Id),
    /// User Ready
    UReady(Id),
    /// User Full
    UFull(UserState), 
    /// User Tree
    UTree(TreeState),
    /// Room Tree 
    RTree(TreeState),
    /// Room Ready 
    RReady(Id),
}


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ClientMessage {

    // pub msg_id: i64,

    pub typ: ::core::option::Option<client_message::MsgType>,
}

/// Nested message and enum types in `ClientMessage`.
pub mod client_message {

    #[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
    pub enum MsgType {

        Open(super::OpenSessionRequest),

        Reconn(super::ReconnectRequest),

        Close(super::CloseSessionRequest),

        CreateX(super::CreateWebrtcTransportRequest),

        ConnX(super::ConnectWebrtcTransportRequest),

        Pub(super::PublishRequest),

        UPub(super::UnPublishRequest),

        Sub(super::SubscribeRequest),

        USub(super::UnSubscribeRequest),

        Mute(super::MuteRequest),

        Layer(super::LayerRequest),

        End(super::EndRequest),

        UpExt(super::UpdateExtRequest),

        UpUTree(super::UpdateTreeRequest),

        UpRTree(super::UpdateTreeRequest),

        Chat(super::ChatRequest),

        BEnd(super::BatchEndRequest),

        // Ack(super::PushAck),
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct OpenSessionRequest {
    pub user_id: String,

    pub room_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ext: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_tree: Option<Vec<UpdateTreeRequest>>,    

    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<bool>, 
    
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub create_x_requests: Option<Vec<CreateWebrtcTransportRequest>>,   
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ReconnectRequest {
    pub session_id: ::prost::alloc::string::String,

    // pub room_cursors: ::prost::alloc::vec::Vec<RoomCursor>,

    // /// last ack seq
    // pub ack_seq: Option<i64>,

    /// 尝试重连的次数，从1开始，依次递增。
    /// 比如 第一次重连是1，第二次是2，不管连接成功与否，每次尝试连接都递增
    pub try_seq: i64,

    /// 上一次重连成功的 try_seq 值，初始值为 0
    pub last_success_seq: i64,

    /// 标志， 固定为 20250901
    pub magic: i32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct RoomCursor {
    
    pub room_id: String,

    pub seq: i64,
}


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MuteRequest {
    
    pub room_id: String,

    pub producer_id: Option<String>,

    pub stream_id: Option<String>,

    pub muted: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LayerRequest {
    
    pub room_id: String,

    pub consumer_id: String,

    pub preferred_layers: ::core::option::Option<PreferredLayers>,

    // pub s_layer: u8,

    // pub t_layer: Option<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EndRequest {
    
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExtRequest {
    pub ext: Option<String>,
    // pub id: Option<String>, // room_id
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTreeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<AStr>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<AStr>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub to: Option<String>, // 为空表示默认房间消息
    pub body: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatchEndRequest {
    
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatchEndResponse {
    
}


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct OpenSessionResponse {
    pub session_id: String,
    /// 连接 Id
    pub conn_id: ::prost::alloc::string::String,

    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub create_x_responses: Option<Vec<CreateWebrtcTransportResponse>>, 
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct CloseSessionResponse {

}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ReconnectResponse {
    /// 连接 Id
    pub conn_id: ::prost::alloc::string::String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct CloseSessionRequest {
    pub room_id: String,
    /// 是否解散房间
    pub end: Option<bool>,
}


// #[derive(serde::Serialize)]
// // #[derive(::dump_derive::Dump)]
// #[allow(clippy::derive_partial_eq_without_eq)]
// #[derive(Clone, PartialEq)]
// #[derive(Debug)]
// pub struct ServerMessageRef {
//     pub msg_type: ::core::option::Option<server_message::MsgType>,
// }


// impl<'a> Into<String> for ServerMessageRef<'a> {
//     fn into(self) -> String {
//         (&self).into()
//     }
// }

// impl<'a, 'b> Into<String> for &'b ServerMessageRef<'a> {
//     fn into(self) -> String {
//         match serde_json::to_string(self) {
//             Ok(v) => v,
//             Err(e) => unreachable!("{e:?}"),
//         }
//     }
// }

/// Nested message and enum types in `ServerMessage`.
// pub mod server_message {
//     #[derive(serde::Serialize)]
//     // #[derive(::dump_derive::Dump)]
//     #[allow(clippy::derive_partial_eq_without_eq)]
//     #[derive(Clone, PartialEq)]
//     #[derive(Debug)]
//     pub enum MsgType {
//         Response(super::Response),

//         ClosedNotice(super::ClosedNotice),

//         // Notice(super::RoomNotice),

//         // ReadyNotice(super::ready_notice::ReadyType),

//         // Ch(super::ChNoticeRef<'a>),

//         // P1(super::PushRef<'a>), // track seq and ack

//         // P2(super::PushRef<'a>), // track seq and immediate ACK optional
//     }
// }

// pub mod ready_notice {
//     #[derive(serde::Deserialize, serde::Serialize)]
//     // #[derive(::dump_derive::Dump)]
//     #[allow(clippy::derive_partial_eq_without_eq)]
//     #[derive(Clone, PartialEq)]
//     #[derive(Debug)]
//     pub enum ReadyType {
//         Room(ReadyNoice),
//     }

//     #[derive(serde::Deserialize, serde::Serialize)]
//     // #[derive(::dump_derive::Dump)]
//     #[allow(clippy::derive_partial_eq_without_eq)]
//     #[derive(Clone, PartialEq)]
//     #[derive(Debug)]
//     pub struct ReadyNoice {
//         pub id: String,
//     }

//     #[derive(serde::Serialize)]
//     // #[derive(::dump_derive::Dump)]
//     #[allow(clippy::derive_partial_eq_without_eq)]
//     #[derive(Clone, PartialEq)]
//     #[derive(Debug)]
//     pub struct ReadyNoicRef<'a> {
//         pub id: &'a str,
//     }
// }



#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ResponseRef<'a> {
    pub status: ::core::option::Option<&'a Status>,

    // pub msg_id: i64,

    pub typ: ::core::option::Option<response::ResponseType>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
pub struct Response {
    pub status: ::core::option::Option<Status>,

    // pub msg_id: i64,

    pub typ: ::core::option::Option<response::ResponseType>,
}

/// Nested message and enum types in `Response`.
pub mod response {
    // use mp_common_rs::proto::ms::service as mspb;

    #[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
    pub enum ResponseType {
        
        Open(super::OpenSessionResponse),

        Reconn(super::ReconnectResponse),

        Close(super::CloseSessionResponse),

        CreateX(super::CreateWebrtcTransportResponse),

        ConnX(super::ConnectWebrtcTransportResponse),

        Pub(super::PublishResponse),

        UPub(super::UnPublishResponse),

        Sub(super::SubscribeResponse),

        USub(super::UnSubscribeResponse),

        Mute(super::MuteResponse),

        Layer(super::LayerResponse),

        End(super::EndResponse),

        UpExt(super::UpdateExtResponse),

        UpUTree(super::UpdateTreeResponse),

        UpRTree(super::UpdateTreeResponse),

        Chat(super::ChatResponse),

        BEnd(super::BatchEndResponse),
    }
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ClosedNoticeRef<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: ::core::option::Option<&'a Status>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
pub struct ClosedNotice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: ::core::option::Option<Status>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct RoomNotice {

    pub room_id: String,

    pub seq: i64,

    pub json: String,

    // #[prost(oneof = "room_notice::NoticeType", tags = "3, 4, 5, 6, 7, 8, 9")]
    // pub notice_type: ::core::option::Option<room_notice::NoticeType>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ChNoticeRef<'a> {

    pub id: &'a str,

    pub seq: i64,

    pub body: &'a str,

    // #[prost(oneof = "room_notice::NoticeType", tags = "3, 4, 5, 6, 7, 8, 9")]
    // pub notice_type: ::core::option::Option<room_notice::NoticeType>,
}

// #[derive(serde::Deserialize, serde::Serialize)]
// // #[derive(::dump_derive::Dump)]
// #[allow(clippy::derive_partial_eq_without_eq)]
// #[derive(Clone, PartialEq)]
// #[derive(Debug)]
// pub struct PushRef<'a> {

//     pub seq: i64,

//     pub btype: i32, // see BodyType

//     pub body: &'a str,
// }


#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct PushAck {
    pub seq: i64,
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct ChatNoticeRef<'a> {

    pub from: &'a str,

    pub to: &'a str,

    pub body: &'a str,
}

#[derive(serde::Deserialize, Clone, PartialEq, Debug)]
pub struct ChatNotice {

    pub from: String,

    pub to: String,

    pub body: String,
}

#[derive(serde::Serialize, Clone, PartialEq, Debug)]
pub struct TreeStateRef<'a> {
    pub path: &'a str,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

#[derive(serde::Deserialize, Clone, PartialEq, Debug)]
pub struct TreeState {
    pub path: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune: Option<bool>,
}

// pub mod room_notice {
    // #[derive(serde::Deserialize, serde::Serialize)]
    // // #[derive(::dump_derive::Dump)]
    // #[allow(clippy::derive_partial_eq_without_eq)]
    // #[derive(Clone, PartialEq)]
    // #[derive(Debug)]
    // pub enum NoticeType {
    //     User(super::UserState),

    // }

    // #[derive(serde::Serialize)]
    // pub enum NoticeTypeRef<'a> {
    //     User(&'a super::UserState),
    //     /// User Tree
    //     UTree(TreeStateRef<'a>),
    //     /// Room Tree
    //     RTree(TreeStateRef<'a>),
    //     // Chat(ChatMsgRef<'a>),
    // }



    // #[derive(serde::Serialize)]
    // pub struct ChatMsgRef<'a> {
    //     pub text: &'a str,

    //     #[serde(skip_serializing_if = "Option::is_none")]
    //     pub from: Option<&'a str>,

    //     #[serde(skip_serializing_if = "Option::is_none")]
    //     pub to: Option<&'a str>,
    // }
// }

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct UserState {

    pub id: String, // user id

    pub online: bool,
    
    pub streams: HashMap<String, Stream>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,

    pub inst_id: String,  // instance id
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct Stream {
    pub seq: i64,

    // pub kind: i32,

    pub stype: i32,

    pub producer_id: String,

    pub muted: bool,
}

/// 创建 WebRtcTransport 请求参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebrtcTransportRequest {

    /// Room Id
    pub room_id: ::prost::alloc::string::String,

    /// 传输通道方向, 见 Direction  
    pub dir: i32,

    /// 媒体类型, 见 MediaKind
    pub kind: i32,

    /// 客户端的 Dtls Parameters
    pub dtls: ::core::option::Option<mediasoup::prelude::DtlsParameters>,
}

/// 创建 WebRtcTransport 响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebrtcTransportResponse {

    /// Transport Id
    pub xid: ::prost::alloc::string::String,

    /// Ice Parameters
    pub ice_param: ::core::option::Option<mediasoup::prelude::IceParameters>,

    /// Ice Candidates
    pub ice_candidates: ::prost::alloc::vec::Vec<mediasoup::prelude::IceCandidate>,

    /// Server Dtls Parameters
    pub dtls: ::core::option::Option<mediasoup::prelude::DtlsParameters>,

}


/// 传输通道方向
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, FromRepr)]
#[repr(i32)]
pub enum Direction {
    /// 发送通道
    Inbound = 0,
    /// 接收通道
    Outbound = 1,
}
impl Direction {
    pub fn value(&self) -> i32 {
        *self as i32
    }
    
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Direction::Inbound => "INBOUND",
            Direction::Outbound => "OUTBOUND",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INBOUND" => Some(Self::Inbound),
            "OUTBOUND" => Some(Self::Outbound),
            _ => None,
        }
    }
}


/// 媒体类型
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MediaKind {
    /// 音频+视频
    AudioVideo = 0,
    /// 音频
    Audio = 1,
    /// 视频
    Video = 2,
}
impl MediaKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            MediaKind::AudioVideo => "AUDIO_VIDEO",
            MediaKind::Audio => "AUDIO",
            MediaKind::Video => "VIDEO",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "AUDIO_VIDEO" => Some(Self::AudioVideo),
            "AUDIO" => Some(Self::Audio),
            "VIDEO" => Some(Self::Video),
            _ => None,
        }
    }
}


/// 连接 WebRtcTransport 请求参数
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq)]
#[derive(Debug)]
pub struct ConnectWebrtcTransportRequest {
    /// Transport Id
    pub xid: ::prost::alloc::string::String,

    /// Client Dtls Parameters
    pub dtls: ::core::option::Option<mediasoup::prelude::DtlsParameters>,
}


/// 发布媒体流请求参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    /// Room Id
    pub room_id: ::prost::alloc::string::String,
    
    /// 发送通道 Transport Id
    pub xid: ::prost::alloc::string::String,
    
    pub stream_id: ::prost::alloc::string::String,
    
    /// 媒体类型, 见 MediaKind
    // pub kind: i32,

    /// 媒体流类型
    pub stype: i32,

    /// Rtp Parameters
    pub rtp: ::core::option::Option<mediasoup::prelude::RtpParameters>,

    /// 音频类型, 见 AudioType
    pub audio_type: i32,

    pub muted: ::core::option::Option<bool>,

}

/// 取消发布媒体流请求参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnPublishRequest {
    /// Room Id
    pub room_id: ::prost::alloc::string::String,

    /// 生产者 Id
    pub producer_id: Option<String>,

    /// 媒体流 Id （和生产者 Id 二选一）
    pub stream_id: Option<String>,

}

/// 订阅媒体流请求参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    /// Room Id
    pub room_id: ::prost::alloc::string::String,

    /// 流id
    pub stream_id: ::prost::alloc::string::String,

    /// 生产者 Id
    pub producer_id: ::prost::alloc::string::String,

    /// 接收通道 Transport Id
    pub xid: ::prost::alloc::string::String,

    /// 空间层/时间层
    pub preferred_layers: ::core::option::Option<PreferredLayers>,

}

/// 取消订阅媒体流请求参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnSubscribeRequest {
    /// Room Id
    pub room_id: ::prost::alloc::string::String,

    /// 媒体流 Id (consumer_id)
    pub consumer_id: ::prost::alloc::string::String,
}

/// 音频订阅类型，视频无需区分
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum AudioType {
    /// 参与选路的音频，客户端推上来音频流
    RoutableAudio = 0,
    /// 独占音频，不参与音频选路，需要单独订阅
    ExclusiveAudio = 1,
    /// 参与选路的房间音频，音量最大的三路流，经过选完路之后的音频
    PriorityRoomAudio = 2,
}

// impl AudioType {
//     /// String value of the enum field names used in the ProtoBuf definition.
//     ///
//     /// The values are not transformed in any way and thus are considered stable
//     /// (if the ProtoBuf definition does not change) and safe for programmatic use.
//     pub fn as_str_name(&self) -> &'static str {
//         match self {
//             AudioType::RoutableAudio => "routable_audio",
//             AudioType::ExclusiveAudio => "exclusive_audio",
//             AudioType::PriorityRoomAudio => "priority_room_audio",
//         }
//     }
//     /// Creates an enum from field names used in the ProtoBuf definition.
//     pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
//         match value {
//             "routable_audio" => Some(Self::RoutableAudio),
//             "exclusive_audio" => Some(Self::ExclusiveAudio),
//             "priority_room_audio" => Some(Self::PriorityRoomAudio),
//             _ => None,
//         }
//     }
// }

/// 空间层/时间层
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreferredLayers {
    /// 空间层
    pub spatial_layer: u32,

    /// 时间层, < 0 则不设置
    pub temporal_layer: i32,
}


/// 连接 WebRtcTransport 响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWebrtcTransportResponse {

}

/// 发布媒体流响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublishResponse {
    
    /// 生产者 Id
    pub producer_id: ::prost::alloc::string::String,
}


/// 取消发布媒体流响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnPublishResponse {
    // /// 状态码和原因
    // pub status: ::core::option::Option<Status>,
}

/// 订阅媒体流响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeResponse {
    /// 消费者 Id
    pub consumer_id: ::prost::alloc::string::String,

    /// Rtp Parameters
    pub rtp: ::core::option::Option<mediasoup::prelude::RtpParameters>,
}

/// 取消订阅媒体流响应参数
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UnSubscribeResponse {

}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MuteResponse {

}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LayerResponse {

}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EndResponse {
    pub updated: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExtResponse {
    
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTreeResponse {
    
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    
}

#[derive( Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
#[repr(i32)]
pub enum StreamType {
    Camera = 1,
    Mic = 2,
    Screen = 3, 
}

impl StreamType {
    pub fn from_value(value: i32) -> Result<Self>{
        Self::from_repr(value).with_context(||format!("invalid StreamType [{value}]"))
    }

    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn value_str(value: i32) -> Result<&'static str>{
        Self::from_value(value).map(|x|x.as_str())
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StreamType::Camera => "camera",
            StreamType::Mic => "mic",
            StreamType::Screen => "screen",
        }
    }
}



// impl TryFrom<i32> for StreamType {
//     type Error = anyhow::Error;

//     fn try_from(value: i32) -> Result<Self, Self::Error> {
//         match value {
//             1 => Ok(Self::Camera),
//             2 => Ok(Self::Mic),
//             3 => Ok(Self::ShareVideo),
//             _ => bail!("unknown stream type [{}]", value)
//         }
//     }
// }

pub fn stream_type_from(value: i32) -> Result<(StreamType, mediasoup::prelude::MediaKind)> {
    match value {
        1 => Ok((StreamType::Camera, mediasoup::prelude::MediaKind::Video)),
        2 => Ok((StreamType::Mic, mediasoup::prelude::MediaKind::Audio)),
        3 => Ok((StreamType::Screen, mediasoup::prelude::MediaKind::Video)),
        _ => bail!("unknown stream type [{}]", value)
    }
}


// #[derive(serde::Serialize)]
// struct MyStruct<T: fmt::Display> {
//     id: u32,
//     #[serde(serialize_with = "display_as_str")]
//     value: T,
// }

// fn display_as_str<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
// where
//     T: fmt::Display,
//     S: serde::Serializer,
// {
//     serializer.collect_str(value)
// }


#[cfg(test)]
mod test {
    use crate::proto::{PacketRef, PacketType};

    #[test]
    fn test() {
        let msg = PacketRef::qos1(1, PacketType::Request, "foo");

        let json = serde_json::to_string(&msg).unwrap();
        println!("{json}");

        let obj: PacketRef = serde_json::from_str(&json).unwrap();
        println!("{obj:?}");
    }

    #[test]
    fn test2() {
        // let json = r#"{"sn":1,"typ":3,"body":"{\"typ\":{\"Open\":{\"user_id\":\"simon2\",\"room_id\":\"room01\",\"user_ext\":\"\",\"user_tree\":[{\"path\":\"simon2.k1\",\"value\":\"222\"}]}}}"}"#;

        // 以下rust代码，为什么 obj1 能正常解析， obj2 却解析失败了呢

        let json = r#"{"sn":1,"typ":3,"body":"{\"typ\":1}"}"#;

        let obj1: Foo = serde_json::from_str(&json).unwrap();
        println!("{obj1:?}");

        let obj2: FooRef = serde_json::from_str(&json).unwrap();
        println!("{obj2:?}");

        #[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
        #[serde(bound(deserialize = "'de: 'a"))]
        pub struct FooRef<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub typ: Option<i32>,  // see PacketType

            #[serde(skip_serializing_if = "Option::is_none")]
            pub sn: Option<i64>,  // seq number

            #[serde(skip_serializing_if = "Option::is_none")]
            pub body: Option<&'a str>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub ack: Option<i64>,  // ack sn
        }

        #[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
        // #[serde(bound(deserialize = "'de: 'a"))]
        struct Foo {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub typ: Option<i32>,  // see PacketType

            #[serde(skip_serializing_if = "Option::is_none")]
            pub sn: Option<i64>,  // seq number

            #[serde(skip_serializing_if = "Option::is_none")]
            pub body: Option<String>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub ack: Option<i64>,  // ack sn
        }
    }

}


