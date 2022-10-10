use crate::error::Error;
use crate::helpers::BoxAny;
use crate::helpers::Named;
use crate::provider::{Provider, ProviderObject, Resolvable};
use futures::future::{abortable, AbortHandle};
use std::any::{type_name, TypeId};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub struct Resolver {
    worker_tx: mpsc::Sender<WorkerMessage>,
    abort: AbortHandle,
}

impl Resolver {
    pub(crate) async fn resolve<S>(&self) -> Result<S, Error>
    where
        S: Resolvable,
    {
        resolve(&self.worker_tx).await
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        self.abort.abort()
    }
}

#[derive(Debug)]
pub struct ResolverBuilder {
    provider_map: BTreeMap<TypeId, Named<Arc<dyn ProviderObject>>>,
}

impl ResolverBuilder {
    pub(crate) fn new() -> Self {
        Self {
            provider_map: BTreeMap::new(),
        }
    }

    pub fn register<P>(&mut self, provider: P) -> &mut Self
    where
        P: Provider,
    {
        let type_id = TypeId::of::<P::Ref>();
        let provider: Arc<dyn ProviderObject> = Arc::new(provider);
        self.provider_map.insert(
            type_id,
            Named {
                name: type_name::<P::Ref>(),
                value: provider,
            },
        );
        self
    }

    pub(crate) fn finalize(self) -> Resolver {
        let (worker_tx, worker_rx) = mpsc::channel(32);
        let worker = Worker {
            provider_map: self.provider_map,
            instance_map: BTreeMap::new(),
        };
        let (task, abort) = abortable(worker.start(worker_tx.clone(), worker_rx));

        tokio::spawn(task);

        Resolver { worker_tx, abort }
    }
}

#[derive(Debug)]
enum WorkerMessage {
    ResolveRequest {
        type_info: Named<TypeId>,
        tx: oneshot::Sender<Result<Arc<BoxAny>, Error>>,
    },
    ProviderCallback {
        type_info: Named<TypeId>,
        result: Result<Arc<BoxAny>, Error>,
    },
}

struct Worker {
    provider_map: BTreeMap<TypeId, Named<Arc<dyn ProviderObject>>>,
    instance_map: BTreeMap<TypeId, Named<InstanceSlot>>,
}

enum InstanceSlot {
    ProviderRunning {
        // We cannot use Arc<Any> because Arc<Any>::downcast<T> requires T: Sized,
        // but we need T: ?Sized to cast Arc<Any> to Arc<dyn T> so we must box Arc<dyn T>
        txs: Vec<oneshot::Sender<Result<Arc<BoxAny>, Error>>>,
    },
    Resolved(Arc<BoxAny>),
}

impl Worker {
    async fn start(
        mut self,
        tx: mpsc::Sender<WorkerMessage>,
        mut rx: mpsc::Receiver<WorkerMessage>,
    ) {
        let r = ResolverRef(tx);
        loop {
            tokio::select! {
              Some(msg) = rx.recv() => {
                tracing::debug!("msg: {:?}", msg);

                match msg {
                  WorkerMessage::ResolveRequest {
                    type_info,
                    tx
                  } => {
                    self.dispatch(&r, type_info, tx);
                  },
                  WorkerMessage::ProviderCallback { type_info, result } => {
                    match result {
                      Ok(instance) => {
                        let replaced = self.instance_map.insert(type_info.value, type_info.map(|_| {
                          InstanceSlot::Resolved(instance.clone())
                        }));
                        match replaced.map(|v| v.value) {
                          Some(InstanceSlot::ProviderRunning { txs }) => {
                            for tx in txs {
                              tx.send(Ok(instance.clone())).ok();
                            }
                          },
                          _ => unreachable!()
                        }
                      },
                      Err(err) => {
                        match self.instance_map.remove(&type_info.value).map(|v| v.value) {
                          Some(InstanceSlot::ProviderRunning { txs }) => {
                            for tx in txs {
                              tx.send(Err(err.clone())).ok();
                            }
                          },
                          _ => unreachable!()
                        }
                      }
                    }
                  }
                }
              }
            }
        }
    }

    fn dispatch(
        &mut self,
        r: &ResolverRef,
        type_info: Named<TypeId>,
        reply_tx: oneshot::Sender<Result<Arc<BoxAny>, Error>>,
    ) {
        let type_id = type_info.value;
        if let Some(slot) = self.instance_map.get_mut(&type_id) {
            match slot.value {
                InstanceSlot::ProviderRunning { ref mut txs } => {
                    txs.push(reply_tx);
                }
                InstanceSlot::Resolved(ref instance) => {
                    reply_tx.send(Ok(instance.clone())).ok();
                }
            }
            return;
        }

        if let Some(provider) = self.provider_map.get(&type_id).cloned() {
            self.instance_map.insert(
                type_id,
                type_info.with_value(InstanceSlot::ProviderRunning {
                    txs: vec![reply_tx],
                }),
            );
            let r = r.clone();
            tokio::spawn(async move {
                let result = provider.value.provide(&r).await;
                r.0.send(WorkerMessage::ProviderCallback {
                    type_info,
                    result: result.map(Arc::new),
                })
                .await
                .ok();
            });
        } else {
            reply_tx
                .send(Err(Error::UnregisteredServiceType(type_info.name)))
                .ok();
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolverRef(mpsc::Sender<WorkerMessage>);

impl ResolverRef {
    pub fn deferred<S>(&self) -> Deferred<S> {
        Deferred::new(self.clone())
    }

    pub async fn resolve<S>(&self) -> Result<S, Error>
    where
        S: Resolvable,
    {
        resolve(&self.0).await
    }
}

async fn resolve<S>(worker_tx: &mpsc::Sender<WorkerMessage>) -> Result<S, Error>
where
    S: Resolvable,
{
    let type_name = type_name::<S>();
    let type_id = TypeId::of::<S>();

    let (tx, rx) = oneshot::channel();
    let req = WorkerMessage::ResolveRequest {
        type_info: Named {
            name: type_name,
            value: type_id,
        },
        tx,
    };
    worker_tx.send(req).await.map_err(|_| Error::WorkerGone)?;
    let res = rx.await.map_err(|_| Error::WorkerGone)?;
    Ok(res.map(|v| v.downcast_ref::<S>().unwrap().clone())?)
}

#[derive(Debug, Clone)]
pub struct Deferred<S> {
    r: ResolverRef,
    _p: PhantomData<S>,
}

impl<S> Deferred<S> {
    fn new(r: ResolverRef) -> Self {
        Self { r, _p: PhantomData }
    }

    pub async fn resolve(&self) -> Result<S, Error>
    where
        S: Resolvable,
    {
        self.r.resolve::<S>().await
    }
}