// #![cfg(not(target_os = "android"))]


use anyhow::Result;
use tokio::{select, sync::mpsc};
use trace_error::anyhow::trace_result;
use vvclient::kit::async_rt;

use vvclient::glue::client::*;
use vvclient::glue::defines::*;
use vvclient::glue::error::ForeignResult;

#[trace_result]
fn main() -> Result<()> {
    
    rustls::crypto::ring::default_provider().install_default()
        .map_err(|e| anyhow::anyhow!("failed to install default crypto [{e:?}]"))?;

    let _guard = vvclient::init_log!(
        &[],
        Some(&vvclient::kit::log::LogFileArgs {
            directory: "/tmp/vvclient".into(),
            name_prefix: "vvclient".into(),
            rolling: tracing_appender::rolling::Rotation::NEVER,
            ..Default::default()
        }),
        Some(true)
    )?;


    async_rt::maybe_init()
        .with_context(||"init async runtime failed")?;
    

    // vvclient::glue::open("ws://127.0.0.1/ws")?;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    async_rt::spawn(async move {
        let r = run_client().await;

        match r {
            Ok(_v) => {
                // let _r = tokio::signal::ctrl_c().await;
                // client.into_finish().await;
            },
            Err(e) => {
                log::error!("{}", trace_fmt!("run_client failed", e));
            },
        }
        
        
        let _r = tx.send(());
    });

    let _r = rx.blocking_recv();
    ::log::debug!("bye!");

    Ok(())
}

#[trace_result]
async fn run_client() -> Result<()> {
    let url = "wss://127.0.0.1:11443/ws";
    // let url = "wss://119.119.119.119:11443/ws";
    // let url = "ws://127.0.0.1:11080/ws";

    let (tx, mut rx) = mpsc::channel(1);

    let room_id = String::from("room01");
    let user_id = String::from("user01");

    let client = vvclient::glue::client::make_signal_client(
        url.into(), 
        SignalConfig {
            user_id: user_id.clone(),
            room_id: room_id.clone(),
            ignore_server_cert: true,
            token: None,
            client_info: None,
        }, 
        std::sync::Arc::new(ListenerImpl {tx}),
    )?;

    // let request = vvclient::glue::defines::CreateXRequest {
    //         room_id: room_id.clone(), 
    //         dir: 0, 
    //         kind: 0, 
    //         dtls: None,
    //     };

    // log::debug!("create_transport {request:?} ...");

    // client.create_x(request, std::sync::Arc::new(()))?;

    // client.create_x(
    //     request, 
    //     std::sync::Arc::new(vvclient::glue::defines::ResponseFn(|response: vvclient::glue::defines::CreateXResponse| {
    //         log::debug!("create_transport response: {:?}", response.xid);
    //         Ok(())
    //     })), 
    //     std::sync::Arc::new(vvclient::glue::defines::FailFn(|code, reason| {
    //         log::debug!("create_transport failed: {:?}", (&code, &reason));
    //         Ok(())
    //     })),
    // )?;


    select! {
        r = tokio::signal::ctrl_c() => {
            log::debug!("got ctrl_c");
            r?;
        }

        r = rx.recv() => {
            r.with_context(||"client already closed")?;
        }
    }

    client.into_finish().await;

    let _r = rx.recv().await;

    Ok(())
}



// struct TypesImpl;

// impl Types for TypesImpl {
//     type Listener = ListenerImpl;

//     type ResponseHandler = ();
// }


struct ListenerImpl {
    tx: mpsc::Sender<()>,
}

// impl Listener for ListenerImpl {
//     fn on_opened(&mut self, session_id: &str) -> Result<()> {
//         log::debug!("got on_opened: session_id [{session_id}]");
//         Ok(())
//     }

//     fn on_closed(&mut self, status: vvclient::proto::Status) -> Result<()> {
//         log::debug!("got on_closed: status [{status:?}]");
//         let _r = self.tx.try_send(());
//         Ok(())
//     }
// }



impl vvclient::glue::client::Listener for ListenerImpl {

    #[tracing::instrument(skip(self))]
    fn on_opened(&self, args: OnOpenedArgs) -> ForeignResult<()> {
        log::debug!("");
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn on_closed(&self, args: OnClosedArgs) -> ForeignResult<()> {
        log::debug!("");
        let _r = self.tx.try_send(());
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn on_created_x(&self, args: OnCreatedXArgs) -> ForeignResult<()> {
        log::debug!("");
        Ok(())
    }
        
    #[tracing::instrument(skip(self))]
    fn on_room_chat(&self, args: OnRoomChatArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_user_chat(&self, args: OnUserChatArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    

    #[tracing::instrument(skip(self))]
    fn on_user_joined(&self, args: OnUserJoinedArgs) -> ForeignResult<()> {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_user_leaved(&self, args: OnUserLeavedArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_add_stream(&self, args: OnAddStreamArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_update_stream(&self, args: OnUpdateStreamArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_remove_stream(&self, args: OnRemoveStreamArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_update_user_tree(&self, args: OnUpdateUserTreeArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_update_room_tree(&self, args: OnUpdateRoomTreeArgs) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }
    
    #[tracing::instrument(skip(self))]
    fn on_room_ready(&self) -> ForeignResult<()>  {
        log::debug!("");
        Ok(())
    }

}

