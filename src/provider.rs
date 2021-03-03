use std::sync::Arc;
use async_trait::async_trait;
use crate::resolver::ResolverRef;
use crate::error::Error;
use crate::helpers::BoxAny;

pub type ProvideResult<T> = anyhow::Result<T>;

#[async_trait]
pub trait Provider: Send + Sync + 'static {
  type Ref: Resolvable;
  async fn provide(&self, resolver: &ResolverRef) -> ProvideResult<Self::Ref>;
}

#[async_trait]
pub(crate) trait ProviderObject: Send + Sync + 'static {
  async fn provide(&self, resolver: &ResolverRef) -> Result<BoxAny, Error>;
}

#[async_trait]
impl<T> ProviderObject for T
  where T: Provider
{
  async fn provide(&self, resolver: &ResolverRef) -> Result<BoxAny, Error> {
    Provider::provide(self, resolver).await
      .map(|v| Box::new(v) as BoxAny)
      .map_err(|err| {
        match err.downcast::<Error>() {
          Ok(err) => err,
          Err(err) => Error::Service(Arc::new(err))
        }
      })
  }
}

pub struct StaticProvider<T>(T);

impl<T> StaticProvider<T> {
  pub fn new(value: T) -> Self {
    StaticProvider(value)
  }
}

#[async_trait]
impl<T> Provider for StaticProvider<T>
where T: Resolvable
{
  type Ref = T;

  async fn provide(&self, _: &ResolverRef) -> ProvideResult<Self::Ref> {
    Ok(self.0.clone())
  }
}

mod private {
  pub trait Sealed {}
}

pub trait Resolvable: Clone + Send + Sync + 'static {}

impl<T> Resolvable for T
  where T: Clone + Send + Sync + ?Sized + 'static
{}