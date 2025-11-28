


use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use std::future::Future;
use tracing::{debug, warn};
use anyhow::{Result, anyhow, Context as AnyhowContext};
use tokio::{ task::JoinHandle, sync::{mpsc::{self, error::{TrySendError, TryRecvError}}, oneshot}};

use crate::kit::async_rt::spawn_with_span;



pub trait ActorHandler : Send + 'static + Sized {
    type Next: Send + 'static; 
    
    type Msg: Send + 'static;

    type Result: Send + 'static;

    fn handle_first(&mut self) -> impl Future<Output = ActionRes> + Send {
        async {
            Ok(Action::None)
        }
    }

    fn wait_next(&mut self) -> impl Future<Output = Self::Next> + Send;

    #[allow(unused_variables)]
    fn handle_next(&mut self, next: Self::Next) -> impl Future<Output = ActionRes> + Send {
        async {
            Ok(Action::None)
        }
    }

    #[allow(unused_variables)]
    fn handle_msg(&mut self, msg: Self::Msg) -> impl Future<Output = ActionRes> + Send {
        async {
            Ok(Action::None)
        }
    }

    fn into_result(self) -> Self::Result;
    
    fn start(self, span: tracing::Span) -> Actor<Self> {
        ActorBuilder::new().build(span, self, )
    }
}


pub struct ActorBuilder<E: ActorHandler> {
    op_tx: mpsc::Sender<Op<E>>,
    op_rx: mpsc::Receiver<Op<E>>,
}

impl<E: ActorHandler> ActorBuilder<E> {
    pub fn new() -> Self {
        let (op_tx, op_rx) = mpsc::channel(128);
        Self { op_tx, op_rx, }
    }

    pub fn weak_invoker(&self) -> WeakInvoker<E> {
        WeakInvoker { op_tx: self.op_tx.downgrade() }
    }

    pub fn build(
        self,
        span: tracing::Span,
        entity: E, 
    ) -> Actor<E>
    where
        E: ActorHandler ,
    {
        // let (op_tx, op_rx) = mpsc::channel(128);
        let op_tx = self.op_tx;
        let op_rx = self.op_rx;

        let is_drop = Arc::new(AtomicBool::new(false));
        let mut task = ActorTask {
            is_drop: is_drop.clone(),
            op_rx,
            actor: entity,
        };
        
        let task_handle = spawn_with_span(span, async move {
            let r = run_actor(&mut task).await;
            if let Err(e) = r {
                warn!("finish with err [{:?}]", e)
            }
            task.actor.into_result()
        });
        
        Actor {
            invoker: Invoker { op_tx},
            wait4completed: Some(Wait4Completed{ task_handle }),
            is_drop,
        }
    }
}

pub enum Action {
    None,
    Finished,
}

pub type ActionRes = Result<Action>;

pub struct Actor<E: ActorHandler> {
    invoker: Invoker<E>,
    wait4completed: Option<Wait4Completed<E>>,
    is_drop: Arc<AtomicBool>,
}

impl<E: ActorHandler> Actor<E> {
    pub fn invoker(&self) -> &Invoker<E> {
        &self.invoker
    }

    pub fn take_completed(&mut self) -> Option<Wait4Completed<E>> {
        self.wait4completed.take()
    }

    pub async fn wait_for_completed(&mut self) -> Result<Option<E::Result>> {
        if let Some(completed) = self.take_completed() {
            Ok(Some(completed.wait_for_completed().await?))
        } else {
            Ok(None)
        }
    }
}

impl<E: ActorHandler> Drop for Actor<E> {
    fn drop(&mut self) {
        self.is_drop.store(true, Ordering::Release);
        let _r = self.invoker.op_tx.try_send(Op::Shutdown);
    }
}



pub struct Wait4Completed<E: ActorHandler> {
    task_handle: JoinHandle<E::Result>,
}

impl<E: ActorHandler> Wait4Completed<E> {
    pub async fn wait_for_completed(self) -> Result<E::Result> {
        let result = self.task_handle.await?;
        Ok(result)
    }
}

pub struct Invoker<E: ActorHandler> {
    op_tx: mpsc::Sender<Op<E>>,
    // none: PhantomData<A>,
}

impl<E: ActorHandler> Clone for Invoker<E> {
    fn clone(&self) -> Self {
        Self { op_tx: self.op_tx.clone() }
    }
}

impl<E: ActorHandler> Invoker<E> {

    pub fn downgrade(&self) -> WeakInvoker<E> {
        WeakInvoker{op_tx: self.op_tx.downgrade()}
    }

    pub async fn invoke<Request, Response>(&self, req: Request) -> Result<Response> 
    where
        Request: Send + 'static,
        Response: Send + 'static,
        E: AsyncHandler<Request, Response = Response> + Send,
    {
        let (tx, rx) = oneshot::channel();
        self.op_tx.send(Op::Invoke(AsyncEnvelope::new(req, tx))).await
        .map_err(|_x|anyhow!("send actor invoke request error"))?;
        let rsp = rx.await.with_context(||"recv actor invoke response but error")?;
        Ok(rsp)
    }

    pub async fn send_msg(&self, msg: E::Msg) -> Result<()> {
        self.op_tx.send(Op::Msg(msg)).await
        .map_err(|_x|anyhow!("send actor msg error"))?;
        Ok(())
    }

    pub fn try_send_msg(&self, msg: E::Msg) -> Result<(), (E::Msg, TrySendError<()>) > {
        let r = self.op_tx.try_send(Op::Msg(msg));

        if let Err(e) = r {
            let e = match e {
                TrySendError::Full(op) => op.try_into_msg().map(|x|(x, TrySendError::Full(())) ),
                TrySendError::Closed(op) =>  op.try_into_msg().map(|x|(x, TrySendError::Closed(()))),
            };

            if let Some(e) = e {
                return Err(e)
            }
        }
        
        Ok(())
    }

    pub async fn shutdown(&self) {
        let _r = self.op_tx.send(Op::Shutdown).await
        .map_err(|_x|anyhow!("send actor shutdown request error"));
    }
}

pub struct WeakInvoker<E: ActorHandler> {
    op_tx: mpsc::WeakSender<Op<E>>,
}

impl<E: ActorHandler> Clone for WeakInvoker<E> {
    fn clone(&self) -> Self {
        Self { op_tx: self.op_tx.clone() }
    }
}

impl<E: ActorHandler> WeakInvoker<E> {
    pub fn upgrade(&self) -> Option<Invoker<E>> {
        self.op_tx.upgrade().map(|op_tx| Invoker{op_tx})
    }
}


async fn run_actor<E>(task: &mut ActorTask<E>) -> Result<()>
where
    E: ActorHandler ,
{
    let r = task.actor.handle_first().await?;
    if let Action::Finished = r {
        debug!("handle first and finished");
        return Ok(());
    }    

    loop {
        let r = {
            let recv_fut = task.op_rx.recv();
            tokio::pin!(recv_fut);
        
            let r = loop {
                tokio::select! {
                    r = task.actor.wait_next() => {
                        task.actor.handle_next(r).await?;
                    }
                    r = &mut recv_fut => {
                        break r;
                    },
                }
            };
            r
        };


        match r {
            Some(op) => {
                let r = handle_op(&mut task.actor, op).await?;
                if let Action::Finished = r {
                    break;
                }
            },
            None => {
                debug!("no one care, done");
                break;
            }
        }
        
        let r = handle_more_op(task).await?;
        if let Action::Finished = r {
            break;
        }
    }
    
    Ok(())
}

async fn handle_more_op<E>(task: &mut ActorTask<E>) -> Result<Action>
where
    E: ActorHandler ,
{
    for _ in 0..8 {
        let recv_op = task.op_rx.try_recv();
        match recv_op {
            Ok(op) => {
                let r = handle_op(&mut task.actor, op).await?;
                if let Action::Finished = r {
                    return Ok(r)
                }
            },
            Err(e) => {
                match e {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => {
                        debug!("recv more but got disconnected");
                        return Ok(Action::Finished)
                    },
                }
            },
        }     
    }

    let is_drop = task.is_drop.load(Ordering::Acquire);
    if is_drop {
        debug!("got drop");
        Ok(Action::Finished)
    } else {
        Ok(Action::None)
    }
}

async fn handle_op<E>(entity: &mut E, op: Op<E>) -> Result<Action>
where
    E: ActorHandler ,

{
    match op {
        Op::Shutdown => {
            debug!("got shutdown");
            return Ok(Action::Finished)
        },
        Op::Invoke(mut envelope) => {
            let _r = envelope.handle(entity).await;
            return Ok(Action::None)
        }
        Op::Msg(msg) => {
            return entity.handle_msg(msg).await
        },
    }
}



struct ActorTask<E> 
where
    E: ActorHandler,
{
    op_rx: mpsc::Receiver<Op<E>>,
    actor: E,
    is_drop: Arc<AtomicBool>,
}


enum Op<E: ActorHandler> {
    Shutdown,
    Invoke(AsyncEnvelope<E>),
    Msg(E::Msg),
}

impl<E: ActorHandler> Op<E> {
    fn try_into_msg(self) -> Option<E::Msg> {
        match self {
            Op::Msg(msg) => Some(msg),
            _ => None
        }
    }
}


#[async_trait::async_trait]
pub trait AsyncHandler<M>
{
    type Response: Send; //: MessageResponse<Self, M>;

    async fn handle(&mut self, msg: M) -> Self::Response;
}


#[async_trait::async_trait]
pub trait AsyncEnvelopeProxy<A> {
    async fn handle(&mut self, act: &mut A);
}


pub struct AsyncEnvelope<A>(Box<dyn AsyncEnvelopeProxy<A> + Send>);

impl<A> AsyncEnvelope<A> {
    pub fn new<M>(msg: M, tx: oneshot::Sender<A::Response>) -> Self
    where
        A: AsyncHandler<M> + Send,
        A::Response: 'static,
        // A::Context: AsyncContext<A>,
        M: Send + 'static, // + Message ,
        // M::Result: Send,
    {
        AsyncEnvelope(Box::new(AsyncEnvelopeReal { msg: Some((msg, tx)) }))
    }

    pub fn with_proxy(proxy: Box<dyn AsyncEnvelopeProxy<A> + Send>) -> Self {
        AsyncEnvelope(proxy)
    }
}

#[async_trait::async_trait]
impl<A> AsyncEnvelopeProxy<A> for AsyncEnvelope<A> 
where
    A: Send,
{
    async fn handle(&mut self, act: &mut A) {
        self.0.handle(act).await
    }
}



pub struct AsyncEnvelopeReal<M, Rsp>
where
    M: Send, 
{
    msg: Option<(M, oneshot::Sender<Rsp>)>
}

#[async_trait::async_trait]
impl<A, M> AsyncEnvelopeProxy<A> for AsyncEnvelopeReal<M, A::Response>
where
    M: Send + 'static, // + Message,
    // M::Result: Send,
    A: AsyncHandler<M> + Send,
    // A::Context: AsyncContext<A>,
{
    async fn handle(&mut self, act: &mut A) {

        if let Some((msg, tx)) = self.msg.take() {
            if tx.is_closed() {
                return;
            }
            
            let rsp = <A as AsyncHandler<M>>::handle(act, msg).await;
            let _r = tx.send(rsp);
        }
    }
}




// // refer from https://stackoverflow.com/questions/70746671/how-to-bind-lifetimes-of-futures-to-fn-arguments-in-rust
// pub trait XFn1<'a, I: 'a, O> {
//     type Output: Future<Output = O> + 'a + Send;
//     fn call_me(&self, session: I) -> Self::Output;
//   }
  
// impl<'a, I: 'a, O, F, Fut> XFn1<'a, I, O> for F
//     where
//     F: Fn(I) -> Fut,
//     Fut: Future<Output = O> + 'a + Send,
// {
//     type Output = Fut;
//     fn call_me(&self, x: I) -> Fut {
//         self(x)
//     }
// }

// pub trait XFn2<'a, I1: 'a, I2: 'a, O> {
//     type Output: Future<Output = O> + 'a + Send;
//     fn call_me(&self, x1: I1, x2: I2) -> Self::Output;
//   }
  
// impl<'a, I1: 'a, I2: 'a, O, F, Fut> XFn2<'a, I1, I2, O> for F
//     where
//     F: Fn(I1, I2) -> Fut,
//     Fut: Future<Output = O> + 'a + Send,
// {
//     type Output = Fut;
//     fn call_me(&self, x1: I1, x2: I2) -> Fut {
//         self(x1, x2)
//     }
// }

// pub async fn handle_first_none<E: ActorEntity>(_entity: &mut E) -> Result<Action> {
//     Ok(Action::None)
// }

// pub fn wait_next_none<E: ActorEntity>(_entity: &mut E) -> Forever {
//     Forever
// }

// pub async fn handle_next_none<E: ActorEntity>(_entity: &mut E, _next: E::Next) -> Result<Action> {
//     Ok(Action::None)
// }

// pub async fn handle_msg_none<E: ActorEntity>(_entity: &mut E, _msg: E::Msg) -> Result<Action> {
//     Ok(Action::None)
// }


// #[derive(Debug, Default)]
// pub struct Forever;

// impl Future for Forever {
//     type Output = ();

//     fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
//         Poll::Pending
//     }
// }











