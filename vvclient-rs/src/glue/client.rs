use std::sync::Arc;

use trace_error::anyhow::trace_result;


use crate::client::{Client as Core, ResponseHandler, Types};
// use crate::glue::defines::{ClientConfig, ConnXRequest, CreateXRequest, IntoHandler as _, ListenerBridge, OnConnXResult, OnCreateXResult, OnEvent};
use crate::glue::defines::*;
use crate::proto;


use super::error::Error;
type Result<T, E = Error> = std::result::Result<T, E>;


#[uniffi::export]
#[trace_result]
pub fn make_client(url: String, config: ClientConfig, on_event: Arc<dyn OnEvent>) -> Result<Client> {

    let core: Core<TypesImpl> = Core::try_new(url, None, config.into(), ListenerBridge(on_event.clone()))?;

    Ok(Client {
        core,
        on_event,
    })
}


#[derive(uniffi::Object)]
pub struct Client {
    core: Core<TypesImpl>,
    on_event: Arc<dyn OnEvent>,
}

impl Client {
    pub async fn into_finish(self) {
        self.core.into_finish().await
    }
}


pub struct TypesImpl;

impl Types for TypesImpl {
    type Listener = ListenerBridge;

    type ResponseHandler = Box<dyn ResponseHandler>;
}



// // #[macro_export]
// macro_rules! define_export_client {
//     (
//         $( ($fn_name:ident, $req_ty:ty, $cb_trait:path) ),* $(,)?
//     ) => {
//         #[uniffi::export]
//         impl Client {
//             $(
//                 #[trace_result]
//                 pub fn $fn_name(
//                     &self,
//                     request: $req_ty,
//                     cb: std::sync::Arc<dyn $cb_trait>
//                 ) -> Result<()> {
//                     let body = request.to_body().with_context(||"to_body failed")?;
//                     self.core.request(body, cb.into_handler()).with_context(||"request faild")?;
//                     Ok(())
//                 }
//             )*
//         }
//     };
// }

// define_export_client!(
//     (create_x, CreateXRequest, OnCreateXResult),
//     (conn_x, ConnXRequest, OnConnXResult),
// );

macro_rules! define_client_result_functions {
    ( $( ($fn_name:ident, $base:ident) ),* $(,)? ) => {
        paste::paste! {
            #[uniffi::export]
            impl Client {
                $(
                    #[trace_result]
                    pub fn [<$fn_name _result>](
                        &self,
                        request: [<$base Request>],
                        cb: std::sync::Arc<dyn [<On $base Result>]>
                    ) -> Result<()> {
                        let body = request.to_body().with_context(||"to_body failed")?;
                        self.core.request(body, cb.into_handler()).with_context(||"request failed")?;
                        Ok(())
                    }
                )*
            }
        }
    };
}

macro_rules! define_client_resolve_functions {
    ( $( ($fn_name:ident, $base:ident) ),* $(,)? ) => {
        paste::paste! {
            #[uniffi::export]
            impl Client {
                $(
                    #[trace_result]
                    pub fn $fn_name(
                        &self,
                        request: [<$base Request>],
                        cb: std::sync::Arc<dyn [<On $base Resolve>]>
                    ) -> Result<()> {
                        let body = request.to_body().with_context(||"to_body failed")?;
                        self.core.request(body, cb.into_handler(self.on_event.clone())).with_context(||"request failed")?;
                        Ok(())
                    }
                )*
            }
        }
    };
}


macro_rules! define_result_handler {
    ($base:ident) => {
        paste::paste! {
            #[uniffi::export(with_foreign)]
            pub trait [<On $base Result>]: Send + Sync {
                fn resolve(&self, response: [<$base Response>]) -> ForeignResult<()>;
                fn reject(&self, code: i32, reason: String) -> ForeignResult<()>;
            }

            impl [<On $base Result>] for () {
                fn resolve(&self, response: [<$base Response>]) -> ForeignResult<()> {
                    log::debug!("success response of [{}], {response:?}", stringify!($base));
                    Ok(())
                }

                fn reject(&self, code: i32, reason: String) -> ForeignResult<()> {
                    log::debug!("fail response of [{}], [{code}]-[{reason}]", stringify!($base));
                    Ok(())
                }
            }

            pub struct [<$base ResponseHandler>] {
                cb: std::sync::Arc<dyn [<On $base Result>]>,
            }

            impl ResponseHandler for [<$base ResponseHandler>] {
                fn on_success(&mut self, response: proto::response::ResponseType) -> anyhow::Result<()> {
                    match response {
                        proto::response::ResponseType::[<$base>](rsp) => {
                            self.cb.resolve(rsp.try_into()?)?;
                            Ok(())
                        }
                        _ => return Err(anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!($base))),
                    }
                }

                fn on_fail(&mut self, code: i32, reason: String) -> anyhow::Result<()> {
                    self.cb.reject(code, reason)?;
                    Ok(())
                }
            }

            impl IntoHandler for std::sync::Arc<dyn [<On $base Result>]> {
                fn into_handler(self) -> Box<dyn ResponseHandler> {
                    Box::new([<$base ResponseHandler>] {
                        cb: self
                    })
                }
            }
        }
    };
}


macro_rules! define_resolve_handler {
    ($base:ident) => {
        paste::paste! {

            #[uniffi::export(with_foreign)]
            pub trait [<On $base Resolve>]: Send + Sync {
                fn resolve(&self, response: [<$base Response>]) -> ForeignResult<()>;
            }

            impl [<On $base Resolve>] for () {
                fn resolve(&self, response: [<$base Response>]) -> ForeignResult<()> {
                    log::debug!("success response of [{}], {response:?}", stringify!($base));
                    Ok(())
                }
            }

            pub struct [<$base ResolveHandler>] {
                cb: std::sync::Arc<dyn [<On $base Resolve>]>,
                on_event: Arc<dyn OnEvent>,
            }

            impl ResponseHandler for [<$base ResolveHandler>] {
                fn on_success(&mut self, response: proto::response::ResponseType) -> anyhow::Result<()> {
                    match response {
                        proto::response::ResponseType::[<$base>](rsp) => {
                            self.cb.resolve(rsp.try_into()?)?;
                            Ok(())
                        }
                        _ => return Err(anyhow::anyhow!("Unexpect response of [{}], {response:?}", stringify!($base))),
                    }
                }

                fn on_fail(&mut self, code: i32, reason: String) -> anyhow::Result<()> {
                    self.on_event.on_request_failed(stringify!($base).into(), code, reason)?;
                    Ok(())
                }
            }

            impl IntoResolveHandler for std::sync::Arc<dyn [<On $base Resolve>]> {
                fn into_handler(self, on_event: Arc<dyn OnEvent>) -> Box<dyn ResponseHandler> {
                    Box::new([<$base ResolveHandler>] {
                        cb: self,
                        on_event,
                    })
                }
            }
        }
    };
}

macro_rules! define_response_handler {
    ($base:ident) => {
        define_result_handler!($base);
        define_resolve_handler!($base);
    };
}

define_response_handler!(CreateX);

define_response_handler!(ConnX);

define_response_handler!(Pub);

define_response_handler!(UPub);

define_response_handler!(Mute);


define_client_result_functions!(
    (create_x, CreateX),
    (conn_x, ConnX),
    (pub_stream, Pub),
    (upub_stream, UPub),
    (mute_stream, Mute),
);

define_client_resolve_functions!(
    (create_x, CreateX),
    (conn_x, ConnX),
    (pub_stream, Pub),
    (upub_stream, UPub),
    (mute_stream, Mute),
);



pub trait IntoResolveHandler {
    fn into_handler(self, on_event: Arc<dyn OnEvent>) -> Box<dyn ResponseHandler>;
}


