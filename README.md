# tower-pipeline

[![Crates.io](https://img.shields.io/crates/v/tower-pipeline.svg)](https://crates.io/crates/tower-pipeline)
[![Docs](https://docs.rs/tower-pipeline/badge.svg)](https://docs.rs/tower-pipeline)
[![dependency status](https://deps.rs/repo/github/davidpdrsn/tower-pipeline/status.svg)](https://deps.rs/repo/github/davidpdrsn/tower-pipeline)
[![Build status](https://github.com/davidpdrsn/tower-pipeline/workflows/CI/badge.svg)](https://github.com/davidpdrsn/tower-pipeline/actions)

A [Tower] [`Service`] combinator that "pipelines" two services.

A `Pipeline` is a [`Service`] consisting of two other [`Service`]s where the response of the
first is the request of the second. This is analogous to [function composition] but for
services.

```rust
use tower_pipeline::PipelineExt;
use tower::{service_fn, BoxError, ServiceExt};

// service that returns the length of a string
let length_svc = service_fn(|input: &'static str| async move {
    Ok::<_, BoxError>(input.len())
});

// service that doubles its input
let double_svc = service_fn(|input: usize| async move {
    Ok::<_, BoxError>(input * 2)
});

// combine our two services
let combined = length_svc.pipeline(double_svc);

// call the service
let result = combined.oneshot("rust").await.unwrap();

assert_eq!(result, 8);
```

[Tower]: https://crates.io/crates/tower
[`Service`]: https://docs.rs/tower-service/0.3.1/tower_service/trait.Service.html
[function composition]: https://en.wikipedia.org/wiki/Function_composition

License: MIT
