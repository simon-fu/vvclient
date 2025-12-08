

use std::sync::Arc;

use trace_error::anyhow::trace_result;

use crate::{client::{ConnectionConfig, JoinAdvanceArgs, JoinConfig, Listener, ResponseHandler}, kit::astr::AStr, proto::{self, mediasoup}};

use super::error::Error;
type Result<T, E = Error> = std::result::Result<T, E>;

use crate::glue::error::ForeignError;
pub type ForeignResult<T, E = ForeignError> = std::result::Result<T, E>;

#[uniffi::export(with_foreign)]
pub trait OnEvent: Send + Sync + 'static {
    fn on_opened(&self, session_id: String) -> ForeignResult<()>;
    
    fn on_closed(&self, code: i32, reason: String) -> ForeignResult<()>;
    
    fn on_request_failed(&self, name: String, code: i32, reason: String) -> ForeignResult<()>;
    
    fn on_room_chat(&self, from: String, to: String, body: String) -> ForeignResult<()>;
    
    fn on_user_chat(&self, from: String, to: String, body: String) -> ForeignResult<()>;

    // fn on_user_joined(&self, user_id: String, camera_muted: Option<bool>, mic_muted: Option<bool>, screen_muted: Option<bool>, user_tree: Vec<TreeNode>) -> ForeignResult<()>;

    fn on_user_joined(&self, user_id: String, args: JoinedArgs) -> ForeignResult<()>;

    fn on_user_leaved(&self, user_id: String) -> ForeignResult<()>;
    
    fn on_add_stream(&self, user_id: String, stype: i32, muted: bool) -> ForeignResult<()>;

    fn on_update_stream(&self, user_id: String, stype: i32, muted: bool) -> ForeignResult<()>;

    fn on_remove_stream(&self, user_id: String, stype: i32) -> ForeignResult<()>; 

    fn on_update_user_tree(&self, user_id: String, op: TreeNode) -> ForeignResult<()>;

    fn on_update_room_tree(&self, op: TreeNode) -> ForeignResult<()>;

    fn on_room_ready(&self) -> ForeignResult<()>;

    fn on_extra_sub(&self, user_id: String, xid: String, stream_id: String, stype: i32, producer_id: String, consumer_id: String, rtp: Option<String>) -> ForeignResult<()>;
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct JoinedArgs {
    pub user_tree: Vec<TreeNode>,
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


// impl OnEvent for () {
//     fn on_opened(&self, session_id: String) -> ForeignResult<()>  {
//         log::debug!("on_opened: session_id [{session_id}]");
//         Ok(())
//     }

//     fn on_closed(&self, code: i32, reason: String) -> ForeignResult<()>  {
//         log::debug!("on_closed: code [{code}], reason [{reason}]");
//         Ok(())
//     }
// }

pub struct ListenerBridge(pub Arc<dyn OnEvent>);

impl Listener for ListenerBridge {
    fn on_opened(&mut self, session_id: &str) -> anyhow::Result<()> {
        self.0.on_opened(session_id.into())?;
        Ok(())
    }

    fn on_closed(&mut self, reason: proto::Status) -> anyhow::Result<()> {
        self.0.on_closed(reason.code, reason.reason)?;
        Ok(())
    }
    
    fn on_room_chat(&mut self, from: &str, to: &str, body: &str) -> anyhow::Result<()> {
        self.0.on_room_chat(from.into(), to.into(), body.into())?;
        Ok(())
    }
    
    fn on_user_chat(&mut self, from: &str, to: &str, body: &str) -> anyhow::Result<()> {
        self.0.on_user_chat(from.into(), to.into(), body.into())?;
        Ok(())
    }

    // fn on_user_joined(&mut self, user_id: &str, camera_muted: Option<bool>, mic_muted: Option<bool>, screen_muted: Option<bool>, tree: Vec<proto::TreeState>) -> anyhow::Result<()> {
    //     let tree: Vec<_> = tree.into_iter()
    //         .map(TreeNode::from)
    //         .collect();

    //     self.0.on_user_joined(user_id.into(), camera_muted, mic_muted, screen_muted, tree)?;
    //     Ok(())
    // }

    fn on_user_joined(&mut self, user_id: AStr, tree: Vec<proto::TreeState>) -> anyhow::Result<()> {
        let user_tree: Vec<_> = tree.into_iter()
            .map(TreeNode::from)
            .collect();

        self.0.on_user_joined(user_id.into(), JoinedArgs { user_tree })?;
        Ok(())
    }
    
    fn on_user_leaved(&mut self, user_id: AStr) -> anyhow::Result<()> {
        self.0.on_user_leaved(user_id.into())?;
        Ok(())
    }
    
    fn on_add_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> anyhow::Result<()> {
        self.0.on_add_stream(user_id.into(), stype, muted)?;
        Ok(())
    }
    
    fn on_update_stream(&mut self, user_id: AStr, stype: i32, muted: bool) -> anyhow::Result<()> {
        self.0.on_update_stream(user_id.into(), stype, muted)?;
        Ok(())
    }
    
    fn on_remove_stream(&mut self, user_id: AStr, stype: i32) -> anyhow::Result<()> {
        self.0.on_remove_stream(user_id.into(), stype)?;
        Ok(())
    }
    
    fn on_update_user_tree(&mut self, user_id: AStr, op: proto::TreeState) -> anyhow::Result<()> {
        self.0.on_update_user_tree(user_id.into(), op.into())?;
        Ok(())
    }
    
    fn on_update_room_tree(&mut self, op: proto::TreeState) -> anyhow::Result<()> {
        self.0.on_update_room_tree(op.into())?;
        Ok(())
    }
    
    fn on_room_ready(&mut self) -> anyhow::Result<()> {
        self.0.on_room_ready()?;
        Ok(())
    }

    fn on_extra_sub(&mut self, user_id: String, xid: String, stream_id: String, stype: i32, producer_id: String, consumer_id: String, rtp: Option<String>) -> anyhow::Result<()> {
        self.0.on_extra_sub(user_id, xid, stream_id, stype, producer_id, consumer_id, rtp)?;
        Ok(())
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct ClientConfig {
    pub user_id: String,

    pub room_id: String,

    pub ignore_server_cert: bool,
}

impl Into<JoinConfig> for ClientConfig {
    fn into(self) -> JoinConfig {
        JoinConfig {
            user_id: self.user_id.into(),
            room_id: self.room_id.into(),
            // advance: Default::default(),
            advance: JoinAdvanceArgs {
                connection: ConnectionConfig {
                    ignore_server_cert: self.ignore_server_cert,
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

// #[uniffi::export(with_foreign)]
// pub trait OnFail: Send + Sync {
//     fn on_fail(
//         &self, 
//         code: i32,
//         reason: String,
//     ) -> ForeignResult<()>;
// }

// pub struct FailFn<F>(pub F);

// impl<F> OnFail for FailFn<F> 
// where 
//     F: Fn(i32, String) -> ForeignResult<()> + Send + Sync,
// {
//     fn on_fail(&self, code:i32, reason:String,) -> ForeignResult<()>  {
//         (self.0)(code, reason)
//     }
// }

// pub struct ResponseFn<F>(pub F);


// #[uniffi::export(with_foreign)]
// pub trait OnCreateXResponse: Send + Sync {
//     fn on_success(
//         &self, 
//         response: CreateXResponse,
//     ) -> ForeignResult<()>;
// }


// impl<F> OnCreateXResponse for ResponseFn<F> 
// where 
//     F: Fn(CreateXResponse) -> ForeignResult<()> + Send + Sync,
// {
//     fn on_success(&self, response: CreateXResponse) -> ForeignResult<()>  {
//         (self.0)(response)
//     }
// }

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct CreateXRequest {

    /// Room Id
    pub room_id: String,

    /// 传输通道方向, 见 Direction  
    pub dir: i32,

    /// 媒体类型, 见 MediaKind
    pub kind: i32,

    /// 客户端的 Dtls Parameters
    pub dtls: Option<String>, // ::core::option::Option<DTLS>, // mediasoup::prelude::DtlsParameters
}

impl CreateXRequest {

    #[trace_result]
    pub fn to_body(&self) -> Result<String> {
        let dtls = match &self.dtls {
            Some(v) => {
                let typed: mediasoup::prelude::DtlsParameters = serde_json::from_str(v)?;
                Some(typed)
            },
            None => None,
        };

        let body = serde_json::to_string(&proto::CreateWebrtcTransportRequestSer {
                room_id: &self.room_id, 
                dir: self.dir, 
                kind: self.kind, 
                dtls: dtls.as_ref(),
            }.into_msg())?;

        Ok(body)
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct CreateXResponse {
    /// Transport Id
    pub xid: ::prost::alloc::string::String,

    /// Ice Parameters
    pub ice_param: Option<String>, // ::core::option::Option<mediasoup::prelude::IceParameters>,

    /// Ice Candidates
    pub ice_candidates: String, //::prost::alloc::vec::Vec<mediasoup::prelude::IceCandidate>,

    /// Server Dtls Parameters
    pub dtls: Option<String>, // ::core::option::Option<mediasoup::prelude::DtlsParameters>,
}

impl TryFrom<proto::CreateWebrtcTransportResponse> for CreateXResponse {
    type Error = Error;

    #[trace_result]
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
pub struct ConnXRequest {

    /// Room Id
    pub xid: String,

    /// 客户端的 Dtls Parameters
    pub dtls: Option<String>, // mediasoup::prelude::DtlsParameters
}

impl ConnXRequest {

    #[trace_result]
    pub fn to_body(&self) -> Result<String> {
        let dtls = match &self.dtls {
            Some(v) => {
                let typed: mediasoup::prelude::DtlsParameters = serde_json::from_str(v)?;
                Some(typed)
            },
            None => None,
        };

        let body = serde_json::to_string(&proto::ConnectWebrtcTransportRequestSer {
                xid: &self.xid,
                dtls: dtls.as_ref(),
            }.into_msg())?;

        Ok(body)
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct ConnXResponse {

}

impl TryFrom<proto::ConnectWebrtcTransportResponse> for ConnXResponse {
    type Error = Error;

    #[trace_result]
    fn try_from(_value: proto::ConnectWebrtcTransportResponse) -> std::result::Result<Self, Self::Error> {

        Ok(Self {

        })
    }
}



// #[uniffi::export(with_foreign)]
// pub trait OnConnXResponse: Send + Sync {
//     fn on_success(
//         &self, 
//         response: ConnXResponse,
//     ) -> ForeignResult<()>;
// }



#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct PubRequest {

    /// Room Id
    pub room_id: String,
    
    /// 发送通道 Transport Id
    pub xid: String,
    
    pub stream_id: String,
    
    /// 媒体类型, 见 MediaKind
    // pub kind: i32,

    /// 媒体流类型
    pub stype: i32,

    /// Rtp Parameters
    pub rtp: Option<String>, // mediasoup::prelude::RtpParameters

    /// 音频类型, 见 AudioType
    pub audio_type: i32,

    pub muted: ::core::option::Option<bool>,
}

impl PubRequest {

    #[trace_result]
    pub fn to_body(&self) -> Result<String> {
        let rtp = match &self.rtp {
            Some(v) => {
                let typed: mediasoup::prelude::RtpParameters = serde_json::from_str(v)?;
                Some(typed)
            },
            None => None,
        };

        let body = serde_json::to_string(&proto::PublishRequestSer {
                room_id: &self.room_id,
                
                xid: &self.xid,
                
                stream_id: &self.stream_id,

                stype: self.stype,

                rtp: rtp.as_ref(),

                audio_type: self.audio_type,

                muted: self.muted,
                
            }.into_msg())?;

        Ok(body)
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct PubResponse {
    pub producer_id: String,
}

impl TryFrom<proto::PublishResponse> for PubResponse {
    type Error = Error;

    #[trace_result]
    fn try_from(value: proto::PublishResponse) -> std::result::Result<Self, Self::Error> {

        Ok(Self {
            producer_id: value.producer_id,
        })
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UPubRequest {
    /// Room Id
    pub room_id: ::prost::alloc::string::String,

    /// 生产者 Id
    pub producer_id: Option<String>,

    /// 媒体流 Id （和生产者 Id 二选一）
    pub stream_id: Option<String>,
}

impl UPubRequest {

    #[trace_result]
    pub fn to_body(&self) -> Result<String> {

        let body = serde_json::to_string(&proto::UnPublishRequestSer {
                room_id: &self.room_id,
                producer_id: self.producer_id.as_deref(),
                stream_id: self.stream_id.as_deref(),
            }.into_msg())?;

        Ok(body)
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct UPubResponse {

}

impl TryFrom<proto::UnPublishResponse> for UPubResponse {
    type Error = Error;

    #[trace_result]
    fn try_from(_value: proto::UnPublishResponse) -> std::result::Result<Self, Self::Error> {

        Ok(Self {

        })
    }
}


#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct MuteRequest {
    
    pub room_id: String,

    pub producer_id: Option<String>,

    pub stream_id: Option<String>,

    pub muted: bool,
}

impl MuteRequest {

    #[trace_result]
    pub fn to_body(&self) -> Result<String> {

        let body = serde_json::to_string(&proto::MuteRequestSer {
                room_id: &self.room_id,
                producer_id: self.producer_id.as_deref(),
                stream_id: self.stream_id.as_deref(),
                muted: self.muted,
            }.into_msg())?;

        Ok(body)
    }
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct MuteResponse {

}

impl TryFrom<proto::MuteResponse> for MuteResponse {
    type Error = Error;

    #[trace_result]
    fn try_from(_value: proto::MuteResponse) -> std::result::Result<Self, Self::Error> {

        Ok(Self {

        })
    }
}


pub trait IntoHandler {
    fn into_handler(self) -> Box<dyn ResponseHandler>;
}

// impl IntoHandler for Arc<dyn OnCreateXResult> {
//     fn into_handler(self) -> Box<dyn ResponseHandler> {
//         Box::new(CreateXResponseHandler {
//             cb: self
//         })
//     }
// }

// #[uniffi::export(with_foreign)]
// pub trait OnCreateXResult: Send + Sync {
//     fn resolve(&self, response: CreateXResponse) -> ForeignResult<()>;

//     fn reject(&self, code: i32, reason: String) -> ForeignResult<()>;
// }


// pub struct CreateXResponseHandler {
//     cb: Arc<dyn OnCreateXResult>,
// }

// impl ResponseHandler for CreateXResponseHandler {
//     fn on_success(self, response: proto::response::ResponseType)-> anyhow::Result<()> {
//         match response {
//             proto::response::ResponseType::CreateX(rsp) => {
//                 self.cb.resolve(rsp.try_into()?)?;
//                 Ok(())
//             } 
//             _ => unreachable!("Not expect response {response:?}") 
//         }
//     }

//     fn on_fail(self, code: i32, reason: String) -> anyhow::Result<()> {
//         self.cb.reject(code, reason)?;
//         Ok(())
//     }
// }





