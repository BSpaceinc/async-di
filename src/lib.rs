mod provider;
pub use provider::{Provider, ProvideResult, Resolvable, StaticProvider};
mod error;
pub use error::Error;
mod resolver;
pub use resolver::{Resolver, ResolverBuilder, ResolverRef, Deferred};
mod helpers;

pub use async_trait::async_trait;
pub type Ref<T> = std::sync::Arc<T>;

#[derive(Debug)]
pub struct Container {
  registry: Resolver,
}

impl Container {
  pub fn new<F>(config: F) -> Self
    where F: FnOnce(&mut ResolverBuilder)
  {
    let mut builder = ResolverBuilder::new();

    config(&mut builder);

    Self {
      registry: builder.finalize()
    }
  }

  pub async fn resolve<S>(&self) -> Result<S, Error>
    where S: Resolvable
  {
    self.registry.resolve().await
  }
}