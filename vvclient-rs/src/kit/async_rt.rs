
use std::{future::Future, sync::OnceLock, time::{Duration, Instant}};

use anyhow::{Context as _, Result, bail};
use tokio::{runtime::Runtime, task::JoinHandle};

pub struct AsyncRT {
    runtime: Runtime,
    // state: Mutex<State>,
}

// struct State {
//     // task: Option<JoinHandle<()>>,
//     guard_tx: Option<oneshot::Sender<()>>,
// }


static INST: OnceLock<AsyncRT> = OnceLock::new();

fn get_inst() -> &'static AsyncRT {
    let Some(inst) = INST.get() else {
        panic!("async runtime is NOT exist")
    };
    inst
}

pub fn maybe_init() -> Result<bool> {
    if INST.get().is_some() {
        Ok(false)
    } else{
        init()?;
        Ok(true)
    }
}

pub fn init() -> Result<()> {
    do_init()?;
    Ok(())
}

pub fn init_block_on<F>(fut: F) -> Result<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let output = do_init()?
        .runtime
        .block_on(fut);
    Ok(output)
}

fn do_init() -> Result<&'static AsyncRT> {
    if INST.get().is_some() {
        bail!("already init");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(||"init tokio runtime failed!")?;

    // let (init_tx, init_rx) = oneshot::channel();
    // let (guard_tx, guard_rx) = oneshot::channel();
    
    // let task = runtime.spawn(async move {
    //     if let Err(_e) = init_rx.await {
    //         return;
    //     }

    //     func(guard_rx).await;

    //     // for num in 1..u64::MAX {
    //     //     log::debug!("num {}", num);
    //     //     tokio::time::sleep(Duration::from_secs(1)).await;
    //     // }
    // });

    let inst = INST.get_or_init(|| AsyncRT {
        runtime,
        // state: Mutex::new(State {
        //     // task: Some(task),
        //     guard_tx: Some(guard_tx),
        // }),
    });

    // let _r = init_tx.send(());

    Ok(inst)
}

// pub fn rt_init_task<F, Fut>(func: F) -> Result<()>
// where
//     F: FnOnce(oneshot::Receiver<()>) -> Fut + Send + Sync + 'static,
//     Fut: Future<Output = ()> + Send + 'static,
// {
//     if INST.get().is_some() {
//         bail!("already init");
//     }

//     let runtime = tokio::runtime::Builder::new_multi_thread()
//         .enable_all()
//         .build()
//         .with_context(||"init tokio runtime failed!")?;

//     let (init_tx, init_rx) = oneshot::channel();
//     let (guard_tx, guard_rx) = oneshot::channel();
    
//     let task = runtime.spawn(async move {
//         if let Err(_e) = init_rx.await {
//             return;
//         }

//         func(guard_rx).await;

//         // for num in 1..u64::MAX {
//         //     log::debug!("num {}", num);
//         //     tokio::time::sleep(Duration::from_secs(1)).await;
//         // }
//     });

//     INST.get_or_init(|| AsyncRT {
//         runtime,
//         state: Mutex::new(State {
//             task: Some(task),
//             guard_tx: Some(guard_tx),
//         }),
//     });

//     let _r = init_tx.send(());

//     Ok(())
// }

// pub async fn rt_wait_for_finish() {
//     let Some(inst) = INST.get() else {
//         return;
//     };

//     let task = {
//         let Some(task) = inst.state.lock().task.take() else {
//             return;
//         };

//         task
//     };

//     let _r = task.await;
// }

#[inline]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    get_inst().runtime.spawn(fut)
}

#[inline]
pub fn spawn_with_span<T>(span: tracing::Span, fut: T) -> tokio::task::JoinHandle<T::Output>
where
    T: std::future::Future + Send + 'static,
    T::Output: Send ,
{
    spawn(tracing::Instrument::instrument(fut, span))
    // tokio::spawn(tracing::Instrument::instrument(fut, span))
}

#[inline]
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[inline]
pub async fn sleep_until(deadline: Instant) {
    tokio::time::sleep_until(deadline.into()).await;
}


