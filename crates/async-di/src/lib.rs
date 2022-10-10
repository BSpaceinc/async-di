mod provider;
pub use provider::{ProvideResult, Provider, Resolvable, StaticProvider};
mod error;
pub use error::Error;
mod resolver;
pub use resolver::{Deferred, Resolver, ResolverBuilder, ResolverRef};
mod helpers;

pub use async_trait::async_trait;
pub type Ref<T> = std::sync::Arc<T>;

#[derive(Debug)]
pub struct Container {
    registry: Resolver,
}

impl Container {
    pub fn new<F>(config: F) -> Self
    where
        F: FnOnce(&mut ResolverBuilder),
    {
        let mut builder = ResolverBuilder::new();

        config(&mut builder);

        Self {
            registry: builder.finalize(),
        }
    }

    pub fn build() -> ContainerBuilder {
        ContainerBuilder {
            resolve_builder: ResolverBuilder::new(),
        }
    }

    pub async fn resolve<S>(&self) -> Result<S, Error>
    where
        S: Resolvable,
    {
        self.registry.resolve().await
    }
}

#[derive(Debug)]
pub struct ContainerBuilder {
    resolve_builder: ResolverBuilder,
}

impl ContainerBuilder {
    pub fn register<P>(&mut self, provider: P) -> &mut Self
    where
        P: Provider,
    {
      self.resolve_builder.register(provider);
      self
    }

    pub fn finalize(self) -> Container {
      Container {
        registry: self.resolve_builder.finalize(),
      }
    }
}
