//! Tower middleware layer that injects an artificial delay into every response.
//! Used to simulate network latency for frontend testing.

use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;

use axum::extract::Request;
use tower::{Layer, Service};

#[derive(Clone, Copy)]
pub struct DelayLayer {
    delay: Duration,
}

impl DelayLayer {
    #[must_use]
    pub const fn new(millis: u64) -> Self { Self { delay: Duration::from_millis(millis) } }
}

impl<S> Layer<S> for DelayLayer {
    type Service = DelayMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service { DelayMiddleware { inner, delay: self.delay } }
}

#[derive(Clone, Copy)]
pub struct DelayMiddleware<S> {
    inner: S,
    delay: Duration,
}

impl<S, B> Service<Request<B>> for DelayMiddleware<S>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;
    type Response = S::Response;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let delay = self.delay;
        let service = self.inner.clone();

        let mut s = mem::replace(&mut self.inner, service);
        let future = s.call(req);

        Box::pin(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            future.await
        })
    }
}

/// Convenience function: produce no-op if delay == 0.
#[must_use]
pub fn delay_middleware(ms: u64) -> Option<DelayLayer> { (ms > 0).then(|| DelayLayer::new(ms)) }
