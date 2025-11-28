


use anyhow::Result;
use tokio::{select, sync::mpsc};
use trace_error::anyhow::trace_result;
use vvclient::{client::{Client, ConnectionConfig, JoinAdvanceArgs, JoinConfig, Listener}, kit::async_rt};

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


    async_rt::try_init()
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


    let client = Client::try_new(
        url, 
        None,
        JoinConfig {
            user_id: "foo".into(),
            room_id: "room01".into(),
            advance: JoinAdvanceArgs {
                connection: ConnectionConfig {
                    ignore_server_cert: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        },
        ListenerImpl {
            tx,
        },
    ).with_context(||"create client failed")?;

    select! {
        r = tokio::signal::ctrl_c() => {
            r?;
        }

        r = rx.recv() => {
            r.with_context(||"client already closed")?;
        }
    }

    let _r = rx.recv().await;

    client.into_finish().await;

    Ok(())
}

struct ListenerImpl {
    tx: mpsc::Sender<()>,
}

impl Listener for ListenerImpl {
    fn on_closed(&mut self, _reason: vvclient::proto::Status) {
        let _r = self.tx.try_send(());
    }
}

