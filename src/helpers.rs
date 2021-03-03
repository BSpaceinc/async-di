use std::fmt;
use std::any::Any;

pub type BoxAny = Box<dyn Any + Send + Sync>;

pub struct Named<T> {
  pub name: &'static str,
  pub value: T,
}

impl<T> Named<T> {
  pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Named<U> {
    Named {
      name: self.name,
      value: f(self.value)
    }
  }

  pub fn with_value<U>(&self, value: U) -> Named<U> {
    Named {
      name: self.name,
      value
    }
  }
}

impl<T> fmt::Debug for Named<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Named")
      .field("name", &self.name)
      .finish()
  }
}

impl<T> Clone for Named<T>
  where T: Clone
{
  fn clone(&self) -> Self {
    Named {
      name: self.name,
      value: self.value.clone(),
    }
  }
}