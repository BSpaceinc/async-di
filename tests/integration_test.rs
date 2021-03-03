use async_di::*;

pub trait ConfigService: Send + Sync {
    fn value(&self) -> i32;
}

pub struct ConfigServiceProvider;

mod impl_config {
    use super::{ConfigService, ConfigServiceProvider};
    use async_di::*;
    use std::sync::Arc;

    struct ConfigServiceImpl {
        value: i32,
    }

    impl ConfigService for ConfigServiceImpl {
        fn value(&self) -> i32 {
            self.value
        }
    }

    #[async_trait::async_trait]
    impl Provider for ConfigServiceProvider {
        type Ref = Arc<dyn ConfigService>;

        async fn provide(&self, _: &ResolverRef) -> ProvideResult<Self::Ref> {
            Ok(Arc::new(ConfigServiceImpl { value: 42 }))
        }
    }
}

#[tokio::test]
async fn test_basic() {
    use std::sync::Arc;

    let container = Container::new(|registry| {
        registry.register(ConfigServiceProvider);
    });
    let config: Arc<dyn ConfigService> = container.resolve().await.unwrap();
    assert_eq!(config.value(), 42)
}

#[tokio::test]
async fn test_dep() {
    use std::sync::Arc;

    pub trait TestService: Send + Sync {
        fn check_config(&self, value: i32) -> bool;
    }

    struct TestServiceImpl(Arc<dyn ConfigService>);

    impl TestService for TestServiceImpl {
        fn check_config(&self, value: i32) -> bool {
            self.0.value() == value
        }
    }

    struct TestServiceProvider;

    #[async_trait::async_trait]
    impl Provider for TestServiceProvider {
        type Ref = Arc<dyn TestService>;

        async fn provide(&self, r: &ResolverRef) -> ProvideResult<Self::Ref> {
            Ok(Arc::new(TestServiceImpl(r.resolve().await?)))
        }
    }

    let container = Container::new(|registry| {
        registry
            .register(ConfigServiceProvider)
            .register(TestServiceProvider);
    });
    let test_service: Arc<dyn TestService> = container.resolve().await.unwrap();

    assert!(test_service.check_config(42))
}

#[tokio::test]
async fn test_deferred() {
    use std::sync::Arc;

    #[async_trait::async_trait]
    pub trait TestService: Send + Sync {
        async fn check_config(&self, value: i32) -> bool;
    }

    struct TestServiceImpl {
        config: Deferred<Arc<dyn ConfigService>>,
    }

    #[async_trait::async_trait]
    impl TestService for TestServiceImpl {
        async fn check_config(&self, value: i32) -> bool {
            let config = self.config.resolve().await.unwrap();
            config.value() == value
        }
    }

    struct TestServiceProvider;

    #[async_trait::async_trait]
    impl Provider for TestServiceProvider {
        type Ref = Arc<dyn TestService>;

        async fn provide(&self, r: &ResolverRef) -> ProvideResult<Self::Ref> {
            Ok(Arc::new(TestServiceImpl {
                config: r.deferred(),
            }))
        }
    }

    let container = Container::new(|registry| {
        registry
            .register(ConfigServiceProvider)
            .register(TestServiceProvider);
    });
    let test_service: Arc<dyn TestService> = container.resolve().await.unwrap();

    assert!(test_service.check_config(42).await)
}
