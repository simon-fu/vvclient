


use anyhow::{Context as _, Result};
use vvclient::{glue, async_rt};


fn main() -> Result<()> {

    let _guard = vvclient::init_log!(
        &[],
        Some(&vvclient::log::LogFileArgs {
            directory: "/tmp/vvclient".into(),
            name_prefix: "vvclient".into(),
            rolling: tracing_appender::rolling::Rotation::NEVER,
            ..Default::default()
        }),
        Some(true)
    )?;


    async_rt::try_init()
        .with_context(||"init async runtime failed")?;
    

    glue::open("ws://127.0.0.1/ws")?;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    async_rt::spawn(async move {
        let _r = tokio::signal::ctrl_c().await;
        let _r = tx.send(());
    });

    let _r = rx.blocking_recv();
    ::log::debug!("bye!");

    Ok(())
}
