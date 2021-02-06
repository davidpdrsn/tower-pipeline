//! A [Tower] [`Service`] combinator that "pipelines" two services.
//!
//! A [`Pipeline`] is a [`Service`] consisting of two other [`Service`]s where the response of the
//! first is the request of the second. This is analogous to [function composition] but for
//! services.
//!
//! ```
//! use tower_pipeline::PipelineExt;
//! use tower::{service_fn, BoxError, ServiceExt};
//!
//! # #[tokio::main]
//! # async fn main() {
//! // service that returns the length of a string
//! let length_svc = service_fn(|input: &'static str| async move {
//!     Ok::<_, BoxError>(input.len())
//! });
//!
//! // service that doubles its input
//! let double_svc = service_fn(|input: usize| async move {
//!     Ok::<_, BoxError>(input * 2)
//! });
//!
//! // combine our two services
//! let combined = length_svc.pipeline(double_svc);
//!
//! // call the service
//! let result = combined.oneshot("rust").await.unwrap();
//!
//! assert_eq!(result, 8);
//! # }
//! ```
//!
//! [Tower]: https://crates.io/crates/tower
//! [`Service`]: tower_service::Service
//! [function composition]: https://en.wikipedia.org/wiki/Function_composition

#![doc(html_root_url = "https://docs.rs/tower-pipeline/0.1.0")]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(broken_intra_doc_links)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use futures_util::ready;
use pin_project_lite::pin_project;
use std::future::Future;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

/// Two services combined where the response of the first is the request of the second.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pipeline<A, B> {
    first: A,
    second: B,
}

impl<A, B> Pipeline<A, B> {
    /// Create a new [`Pipeline`] from two [`Service`]s.
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }

    /// Get a reference to the first service.
    pub fn first_as_ref(&self) -> &A {
        &self.first
    }

    /// Get a mutable reference to the first service.
    pub fn first_as_mut(&mut self) -> &mut A {
        &mut self.first
    }

    /// Consume `self`, returning the first service
    pub fn into_first(self) -> A {
        self.first
    }

    /// Get a reference to the second service.
    pub fn second_as_ref(&self) -> &B {
        &self.second
    }

    /// Get a mutable reference to the second service.
    pub fn second_as_mut(&mut self) -> &mut B {
        &mut self.second
    }

    /// Consume `self`, returning the second service
    pub fn into_second(self) -> B {
        self.second
    }
}

impl<R, A, B> Service<R> for Pipeline<A, B>
where
    A: Service<R>,
    B: Service<A::Response> + Clone,
    A::Error: Into<B::Error>,
{
    type Response = B::Response;
    type Error = B::Error;
    type Future = ResponseFuture<R, A, B>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.first.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: R) -> Self::Future {
        ResponseFuture {
            state: State::FirstFuturePending {
                future: self.first.call(req),
            },
            second: Some(self.second.clone()),
        }
    }
}

pin_project! {
    /// Response future of [`Pipeline`].
    pub struct ResponseFuture<R, A, B>
    where
        A: Service<R>,
        B: Service<A::Response>,
    {
        #[pin]
        state: State<R, A, B>,
        second: Option<B>,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<R, A, B>
    where
        A: Service<R>,
        B: Service<A::Response>,
    {
        FirstFuturePending { #[pin] future: A::Future },
        PollReadySecond { first_res: Option<A::Response>, second: B },
        SecondFuturePending { #[pin] future: B::Future },
    }
}

impl<R, A, B> Future for ResponseFuture<R, A, B>
where
    A: Service<R>,
    B: Service<A::Response>,
    A::Error: Into<B::Error>,
{
    type Output = Result<B::Response, B::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            let new_state = match this.state.as_mut().project() {
                StateProj::FirstFuturePending { future } => {
                    let first_res = ready!(future.poll(cx).map_err(Into::into)?);
                    let second = this.second.take().unwrap();
                    State::PollReadySecond {
                        first_res: Some(first_res),
                        second,
                    }
                }

                StateProj::PollReadySecond { first_res, second } => {
                    let _ready: () = ready!(second.poll_ready(cx)?);
                    State::SecondFuturePending {
                        future: second.call(first_res.take().unwrap()),
                    }
                }

                StateProj::SecondFuturePending { future } => return future.poll(cx),
            };

            this.state.set(new_state);
        }
    }
}

/// An extension trait for easily pipelining [`Service`]s.
pub trait PipelineExt<R>: Service<R> {
    /// Construct a [`Pipeline`].
    fn pipeline<B>(self, second: B) -> Pipeline<Self, B>
    where
        Self: Service<R> + Sized,
        B: Service<Self::Response> + Clone,
        Self::Error: Into<B::Error>;
}

impl<R, T> PipelineExt<R> for T
where
    T: Service<R>,
{
    fn pipeline<B>(self, second: B) -> Pipeline<Self, B>
    where
        Self: Service<R> + Sized,
        B: Service<Self::Response> + Clone,
        Self::Error: Into<B::Error>,
    {
        Pipeline::new(self, second)
    }
}
