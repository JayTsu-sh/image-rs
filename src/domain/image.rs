//! Domain-level opaque image handle.
//!
//! The domain layer must not know about `cv::Mat` or any pixel representation.
//! `ImageBuffer` is therefore a type-erased smart container: infrastructure
//! adapters put their concrete pixel buffer in (`cv::Mat`, `image::DynamicImage`,
//! …) and downcast on the way out. The application layer only ever moves it
//! between ports — it never inspects the contents.
//!
//! This preserves *zero-copy* across the pipeline: each adapter operates on
//! the same heap-allocated buffer the previous adapter produced; the buffer
//! is never serialized to bytes between operations.

use std::any::{Any, TypeId};

pub struct ImageBuffer {
    inner: Box<dyn Any + Send>,
    /// Captured at construction time so the application layer can log the
    /// concrete adapter without downcasting.
    type_name: &'static str,
}

impl ImageBuffer {
    pub fn new<T: Send + 'static>(value: T) -> Self {
        Self {
            inner: Box::new(value),
            type_name: std::any::type_name::<T>(),
        }
    }

    pub fn type_name(&self) -> &'static str { self.type_name }

    pub fn type_id(&self) -> TypeId { (*self.inner).type_id() }

    pub fn downcast<T: 'static>(self) -> Result<Box<T>, Self> {
        match self.inner.downcast::<T>() {
            Ok(b) => Ok(b),
            Err(inner) => Err(Self { inner, type_name: self.type_name }),
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.inner.downcast_mut::<T>()
    }
}

impl std::fmt::Debug for ImageBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageBuffer").field("type", &self.type_name).finish()
    }
}
