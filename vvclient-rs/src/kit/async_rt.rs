
use std::{future::Future, sync::OnceLock, thread, time::{Duration, Instant}};

use anyhow::{Context as _, Result, bail, anyhow};
use tokio::{runtime::{Builder, Handle, Runtime, RuntimeFlavor}, task::{self, JoinHandle}};
use std::sync::mpsc as sync_mpsc;


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

    // 1. 获取系统可用的并行度（逻辑核心数）
    // 如果获取失败（极少见），默认回退到 1
    let core_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    // 2. 至少2个线程，避免 spawn_and_blocking_wait 出错。
    let worker_threads = core::cmp::max(core_count, 2);
    

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
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


/// 【全隔离版】
/// 启动一个新的 OS 线程和临时的 Tokio Runtime 来执行任务。
/// 
/// # 优点
/// - **绝对安全**：由于环境隔离，永远不会发生死锁，哪怕在 `current_thread` 运行时中调用。
/// - **通用**：不依赖当前上下文是否存在 Runtime。
/// 
/// # 缺点
/// - **开销**：每次调用都有创建线程和 Runtime 的成本。
pub fn spawn_thread_and_blocking_wait<F>(fut: F, timeout: Option<Duration>) -> Result<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{

    // 使用 std channel 进行同步线程间的通信
    let (tx, rx) = sync_mpsc::channel();

    // 1. 启动独立的 OS 线程
    thread::spawn(move || {
        // 2. 创建轻量级 Runtime (Current Thread 模式启动很快)
        let rt = match Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(_) => return, // Runtime 创建失败
        };

        // 3. 执行任务
        rt.block_on(async {
            let result = fut.await;
            // 任务完成，尝试发送结果。
            // 如果接收端已超时断开，这里会忽略错误 (send error)，达到 "detach" 效果
            let _ = tx.send(result);
        });
    });

    // 4. 阻塞等待结果
    match timeout {
        Some(duration) => {
            rx.recv_timeout(duration).map_err(|e| match e {
                sync_mpsc::RecvTimeoutError::Timeout => {
                    anyhow!("Operation timed out after {:?} (task continues in background)", duration)
                }
                sync_mpsc::RecvTimeoutError::Disconnected => {
                    anyhow!("Background thread panicked or runtime failed")
                }
            })
        }
        None => {
            rx.recv().map_err(|_| anyhow!("Background thread panicked"))
        }
    }
}

pub fn spawn_and_maybe_blocking_wait<F>(fut: F, timeout: Option<Duration>) -> Result<Option<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    
    match timeout {
        None => {
            // === 场景 1: 不等待 (Fire and Forget) ===

            spawn(async move {
                // 只需要执行，不需要管结果
                let _r = fut.await; 
            });
            return Ok(None);
        },

        Some(timeout) => {
            // === 场景 2: 需要等待 (Blocking Wait) ===

            Ok(Some(spawn_and_blocking_wait(fut, timeout)?))
        },
    }
}

/// 【Tokio 环境版】 
pub fn spawn_and_blocking_wait<F>(fut: F, timeout: Duration) -> Result<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    use std::sync::mpsc;

    if let Ok(handle) = Handle::try_current() {
        if handle.runtime_flavor() == RuntimeFlavor::CurrentThread {
            return spawn_thread_and_blocking_wait(fut, Some(timeout));
        }
    }

    // let duration = timeout;
    let (tx, rx) = mpsc::channel();

    // 启动任务，执行完后把结果传回来
    spawn(async move {
        let result = fut.await;
        // 忽略发送错误（如果接收端超时走人了，这里就不管了）
        let _ = tx.send(result);
    });

    // 定义统一的阻塞等待逻辑（闭包）
    let wait_logic = || -> Result<F::Output> {
        match rx.recv_timeout(timeout) {
            Ok(val) => Ok(val), // 成功拿到结果
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 超时报错，但后台任务依然会继续跑 (Detach)
                Err(anyhow!("Task timed out after {:?}", timeout))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // 任务 Panic 了
                Err(anyhow!("Task panicked"))
            }
        }
    };


    // 智能判断上下文：
    // 1. 如果当前线程是 Tokio 异步工作线程 -> 必须用 block_in_place 让出 CPU。
    // 2. 如果当前线程是普通线程 (main/std::thread) -> 直接阻塞即可。
    if Handle::try_current().is_ok() {
        // 注意：block_in_place 必须在 Multi-Thread Runtime 下才有效
        // 如果在 Current-Thread Runtime 下调用会 Panic。
        // 既然你前提假定是 Multi-Thread，这里是安全的。
        task::block_in_place(wait_logic)
    } else {
        wait_logic()
    }
}


#[inline]
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[inline]
pub async fn sleep_until(deadline: Instant) {
    tokio::time::sleep_until(deadline.into()).await;
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    get_inst().runtime.block_on(future)
}
