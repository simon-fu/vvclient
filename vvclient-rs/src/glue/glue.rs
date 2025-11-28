


use std::sync::Arc;
use std::time::Duration;
use anyhow::Context;
use tracing::{Instrument, Level};

use crate::kit::async_rt;

// use crate::error::ClientError;
// type Result<T, E = ClientError> = std::result::Result<T, E>;
type Result<T, E = Error> = std::result::Result<T, E>;
use super::error::Error;
use crate::glue::error::ForeignError;

// #[derive(uniffi::Object)]
// #[derive(Debug, thiserror::Error)]
// #[error("{e:?}")] // default message is from anyhow.
// pub struct MyError {
//     e: anyhow::Error,
// }

// impl MyError {
//     pub fn inner(&self) -> &anyhow::Error {
//         &self.e
//     }
// }

// #[uniffi::export]
// impl MyError {
//     fn message(&self) -> String {
//         format!("{}", self.e)
//         // self.to_string()
//     }

//     fn debug(&self) -> String {
//         format!("{:?}", self.e)
//     }
// }

// impl From<anyhow::Error> for MyError {
//     fn from(e: anyhow::Error) -> Self {
//         Self { e }
//     }
// }

#[uniffi::export]
pub fn open(url: &str) -> Result<()> {

    let url: String = url.into();

    let span = tracing::span!(parent: None, Level::ERROR, "open", s="abc");
    async_rt::spawn(async move {
        for num in 1..u64::MAX {
            log::info!("info: num {}: {url}", num);
            log::debug!("debug: num {}: {url}", num);
            async_rt::sleep(Duration::from_secs(1)).await;
        }
    }.instrument(span));

    Ok(())
}

#[uniffi::export]
pub fn open_with_cb(url: &str, callback: Option<Arc<dyn CbListener>>) -> Result<()> {
    
    let url: String = url.into();

    let span = tracing::span!(parent: None, Level::ERROR, "open", s="abc");
    async_rt::spawn(async move {
        for num in 1..u64::MAX {
            log::info!("info: num {}: {url}", num);
            log::debug!("debug: num {}: {url}", num);
            if let Some(callback) = &callback {
                let ev = if num % 2 == 0 {
                    Event::User {
                        name: format!("user{}", num),
                    }
                } else {
                    Event::Even1(format!("{}", num))
                };
                log::debug!("callback event {:?}", ev);
                callback.on_event(ev);
                let r = callback.on_oop();
                log::debug!("on_oop: return {r:?}");
            }

            async_rt::sleep(Duration::from_secs(1)).await;
        }
    }.instrument(span));

    Ok(())
}

#[uniffi::export]
pub fn close() {
    log::info!("close");
}

#[uniffi::export]
fn oops() -> Result<()> {
    let err = anyhow::Error::msg("oh,oops");
    Err(err)
        .with_context(||"context1")
        .with_context(||"context2")
        .map_err(|e|e.into())
}

// #[uniffi::export]
#[uniffi::export(with_foreign)]
pub trait CbListener: Send + Sync {
    fn on_event(&self, event: Event) ;
    fn on_user(&self, user: User);
    fn on_oop(&self) -> Result<(), ForeignError>;
}



#[derive(uniffi::Enum)]
#[derive(Debug)]
pub enum Event {
    Even1(String),
    User {
        name: String
    },
    // Foo(Foo)
}

#[derive(uniffi::Record)]
#[derive(Debug)]
pub struct User {
    bar: String
}


// 自动生成脚手架代码，比如 kotlin 生成 jni 代码。
uniffi::setup_scaffolding!();

