use thiserror::Error;
use std::sync::Arc;

#[derive(Error, Debug, Clone)]
pub enum Error {
  #[error("container gone")]
  ContainerGone,
  #[error("resolve worker gone")]
  WorkerGone,
  #[error("unregistered service type: {0}")]
  UnregisteredServiceType(&'static str),
  #[error("service: {0}")]
  Service(Arc<anyhow::Error>),
}