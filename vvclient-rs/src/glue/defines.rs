use core::fmt;

use strum_macros::FromRepr;
use trace_error::anyhow::trace_result;

use crate::{client::defines::{ClientInfo as ClientInfoInner, ConnectionConfig, JoinAdvanceArgs, JoinConfig}, proto::{self, fmt_writer::JsonDisplay, mediasoup}};

use anyhow::{Result, Error};
// use super::error::Error;
// type Result<T, E = Error> = std::result::Result<T, E>;



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct SignalConfig {
    pub user_id: String,

    pub room_id: String,

    pub ignore_server_cert: bool,

    pub token: Option<String>,

    pub client_info: Option<ClientInfo>,
}

impl Into<JoinConfig> for SignalConfig {
    fn into(self) -> JoinConfig {
        let client_info = self.client_info.map(|ci| {
            ClientInfoInner {
                platform: ci.platform.map(Into::into),
                sdk_name: ci.sdk_name.map(Into::into),
                sdk_version: ci.sdk_version.map(Into::into),
                device: ci.device.map(|m| m.into_iter().map(|(k, v)| (k.into(), v.into())).collect()),
            }
        });
        JoinConfig {
            user_id: self.user_id.into(),
            room_id: self.room_id.into(),
            // advance: Default::default(),
            advance: JoinAdvanceArgs {
                connection: ConnectionConfig {
                    ignore_server_cert: self.ignore_server_cert,
                    ..Default::default()
                },
                token: self.token.map(Into::into),
                client_info,
                ..Default::default()
            },
        }
    }
}

#[derive(uniffi::Record)]
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub platform: Option<String>,
    pub sdk_name: Option<String>,
    pub sdk_version: Option<String>,
    pub device: Option<std::collections::HashMap<String, String>>,
}

pub trait ToBody {

    fn to_body_typed(&self) -> Result<impl serde::Serialize>;

    #[trace_result]
    fn to_body_json(&self) -> Result<String> {
        let body = serde_json::to_string(&self.to_body_typed()?)?;
        Ok(body)
    }


    #[trace_result]
    fn to_body(&self) -> Result<impl serde::Serialize + fmt::Display> {
        Ok(JsonDisplay(self.to_body_typed()?))
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct Status {
    pub code :i32,
    pub reason: String,
}

impl Default for Status {
    fn default() -> Self {
        Self::success()
    }
}

impl Status {
    pub fn success() -> Self {
        Self {
            code: 0,
            reason: "OK".into(),
        }
    }
}

impl From<proto::Status> for Status {
    fn from(value: proto::Status) -> Self {
        Self {
            code: value.code,
            reason: value.reason,
        }
    }
}

impl From<Status> for proto::Status {
    fn from(value: Status) -> Self {
        Self {
            code: value.code,
            reason: value.reason,
        }
    }
}

#[derive(uniffi::Enum)]
#[derive( Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
#[repr(i32)]
pub enum StreamType {
    Camera = 1,
    Mic = 2,
    Screen = 3, 
}

impl StreamType {
    pub fn from_value(value: i32) -> Result<Self>{
        Ok(proto::StreamType::from_value(value)?.into())
    }

    pub fn value(&self) -> i32 {
        *self as i32
    }

    pub fn value_str(value: i32) -> Result<&'static str>{
        proto::StreamType::value_str(value)
    }

    pub fn as_str(&self) -> &'static str {
        proto::StreamType::from(*self).as_str()
    }
}



impl From<proto::StreamType> for StreamType {
    fn from(value: proto::StreamType) -> Self {
        match value {
            proto::StreamType::Camera => Self::Camera,
            proto::StreamType::Mic => Self::Mic,
            proto::StreamType::Screen => Self::Screen,
        }
    }
}

impl From<StreamType> for proto::StreamType {
    fn from(value: StreamType) -> Self {
        match value {
            StreamType::Camera => proto::StreamType::Camera,
            StreamType::Mic => proto::StreamType::Mic,
            StreamType::Screen => proto::StreamType::Screen,
        }
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct TreeNode {
    pub path: String,
    pub value: Option<String>,
    pub prune: Option<bool>,
}

impl From<proto::TreeState> for TreeNode {
    fn from(value: proto::TreeState) -> Self {
        Self {
            path: value.path,
            value: value.value,
            prune: value.prune,
        }
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct ChatArgs {
    pub from: String,
    pub to: String,
    pub body: String,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct OnCreateXResponse {
    /// Transport Id
    pub xid: ::prost::alloc::string::String,

    /// Ice Parameters
    pub ice_param: Option<String>, // ::core::option::Option<mediasoup::prelude::IceParameters>,

    /// Ice Candidates
    pub ice_candidates: String, //::prost::alloc::vec::Vec<mediasoup::prelude::IceCandidate>,

    /// Server Dtls Parameters
    pub dtls: Option<String>, // ::core::option::Option<mediasoup::prelude::DtlsParameters>,
}

impl TryFrom<proto::response::ResponseType> for OnCreateXResponse {
    type Error = Error;

    #[trace_result]
    fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
        let value = match response {
            proto::response::ResponseType::CreateX(v) => v,
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(Sub));
                return Err(e)
            },
        };

        Self::try_from(value)
    }
}

impl TryFrom<proto::CreateWebrtcTransportResponse> for OnCreateXResponse {
    type Error = Error;

    fn try_from(value: proto::CreateWebrtcTransportResponse) -> std::result::Result<Self, Self::Error> {
        let ice_param = match &value.ice_param {
            Some(v) => {
                let j = serde_json::to_string(v)?;
                Some(j)
            },
            None => None,
        };

        let ice_candidates = serde_json::to_string(&proto::IterSer(value.ice_candidates.iter().map(|x|proto::IceCandidateSer::from(x))))?;
        // let ice_candidates = serde_json::to_string(&value.ice_candidates)?;

        let dtls = match &value.dtls {
            Some(v) => {
                let j = serde_json::to_string(v)?;
                Some(j)
            },
            None => None,
        };

        Ok(Self {
            xid: value.xid, 
            ice_param, 
            ice_candidates, 
            dtls,
        })
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct ConnXCall {

    /// Room Id
    pub xid: String,

    /// 客户端的 Dtls Parameters
    pub dtls: Option<String>, // mediasoup::prelude::DtlsParameters
}

impl ToBody for ConnXCall {

    #[trace_result]
    fn to_body_typed(&self) -> Result<impl serde::Serialize> {
        let dtls = match &self.dtls {
            Some(v) => {
                let typed: mediasoup::prelude::DtlsParameters = serde_json::from_str(v)?;
                Some(typed)
            },
            None => None,
        };

        Ok(proto::ConnectWebrtcTransportRequestSer {
                xid: &self.xid,
                dtls,
            }.into_msg())
    }
}



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct ConnXReturn {

}

impl TryFrom<proto::response::ResponseType> for ConnXReturn {
    type Error = Error;

    #[trace_result]
    fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
        match response {
            proto::response::ResponseType::ConnX(_rsp) => {
                Result::<_>::Ok(Self {

                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(ConnX));
                return Err(e)
            },
        }
    }
}



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct PubCall {
    pub x_id: String,
    pub stype: StreamType,
    pub rtp: Option<String>,
    pub muted: Option<bool>,
}

impl ToBody for PubCall {

    #[trace_result]
    fn to_body_typed(&self) -> Result<impl serde::Serialize> {
        let rtp = match &self.rtp {
            Some(v) => {
                let typed: mediasoup::prelude::RtpParameters = serde_json::from_str(v)?;
                Some(typed)
            },
            None => None,
        };

        let body = proto::PublishRequestSer {
                room_id: "".into(),
                
                xid: &self.x_id,
                
                stream_id: self.stype.as_str(),

                stype: self.stype.value(),

                rtp,

                audio_type: 0,

                muted: self.muted,
                
            }.into_msg();

        Ok(body)
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct PubReturn {
    pub producer_id: String,
}

impl TryFrom<proto::response::ResponseType> for PubReturn {
    type Error = Error;

    #[trace_result]
    fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
        match response {
            proto::response::ResponseType::Pub(rsp) => {
                Result::<_>::Ok(Self {
                    producer_id: rsp.producer_id,
                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(Pub));
                return Err(e)
            },
        }
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UPubCall {
    pub stype: StreamType,
}

impl ToBody for UPubCall {

    #[trace_result]
    fn to_body_typed(&self) -> Result<impl serde::Serialize> {

        let body = proto::UnPublishRequestSer {
                room_id: "".into(),

                producer_id: None,
                
                stream_id: Some(self.stype.as_str()),
                
            }.into_msg();

        Ok(body)
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UPubReturn {
    
}

impl TryFrom<proto::response::ResponseType> for UPubReturn {
    type Error = Error;

    #[trace_result]
    fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
        match response {
            proto::response::ResponseType::UPub(_rsp) => {
                Result::<_>::Ok(Self {
                    
                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(UPub));
                return Err(e)
            },
        }
    }
}



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct MuteCall {
    
    pub stype: StreamType,

    pub muted: bool,
}

impl ToBody for MuteCall {
    #[trace_result]
    fn to_body_typed(&self) -> Result<impl serde::Serialize> {
        let body = proto::MuteRequestSer {
                room_id: "",
                producer_id: None,
                stream_id: Some(self.stype.as_str()),
                muted: self.muted,
            }.into_msg();
        Ok(body)
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct MuteReturn {

}

impl TryFrom<proto::response::ResponseType> for MuteReturn {
    type Error = Error;

    #[trace_result]
    fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
        match response {
            proto::response::ResponseType::Mute(_rsp) => {
                Result::<_>::Ok(Self {
                    
                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(Mute));
                return Err(e)
            },
        }
    }
}



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct SubCall {
    pub user_id: String,
    pub stype: StreamType,
}

// impl ToBody for SubCall {

//     #[trace_result]
//     fn to_body_typed(&self) -> Result<impl serde::Serialize> {
//         let body = proto::SubscribeRequestSer {

                
//             }.into_msg();

//         Ok(body)
//     }
// }


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct SubReturn {
    pub x_id: String,

    pub consumer_id: String,

    pub producer_id: String,

    pub stream_id: String,

    /// Rtp Parameters
    pub rtp: Option<String>, // Option<mediasoup::prelude::RtpParameters>,
}

impl SubReturn {
    pub fn try_new(x_id: String, producer_id: String, stream_id: String, response: proto::response::ResponseType) -> Result<Self> {
        match response {
            proto::response::ResponseType::Sub(rsp) => {
                Result::<_>::Ok(Self {
                    consumer_id: rsp.consumer_id,
                    rtp: rsp.rtp.map(|x|serde_json::to_string(&x)).transpose()?,
                    x_id, 
                    producer_id,
                    stream_id,
                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(Sub));
                return Err(e)
            },
        }
    }
}

// impl TryFrom<proto::response::ResponseType> for SubReturn {
//     type Error = Error;

//     #[trace_result]
//     fn try_from(response: proto::response::ResponseType) -> std::result::Result<Self, Self::Error> {
//         match response {
//             proto::response::ResponseType::Sub(rsp) => {
//                 Result::<_>::Ok(Self {
//                     consumer_id: rsp.consumer_id,
//                     rtp: rsp.rtp.map(|x|serde_json::to_string(&x)).transpose()?,
//                 })
//             }
//             _ => {
//                 let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(Sub));
//                 return Err(e)
//             },
//         }
//     }
// }


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UnsubCall {
    pub consumer_id: String,
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UnsubReturn {
    
}

impl UnsubReturn {
    pub fn try_new(response: proto::response::ResponseType) -> Result<Self> {
        match response {
            proto::response::ResponseType::USub(_rsp) => {
                Result::<_>::Ok(Self {
                    
                })
            }
            _ => {
                let e = anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!(USub));
                return Err(e)
            },
        }
    }
}
