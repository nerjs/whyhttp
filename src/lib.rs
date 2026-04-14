#![doc = include_str!("../README.md")]

use std::{
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, mpsc::Sender},
};

use crate::{
    matchers::Matcher,
    print::print_report,
    server::run_server,
    worker::{ExpectationId, SharedWorker},
};

mod expectation;
mod io;
mod matchers;
mod print;
mod reports;
pub mod server;
mod worker;

struct Inner {
    worker: SharedWorker,
    addr: SocketAddr,
    server_shutdown_tx: Sender<()>,
}

impl Deref for Inner {
    type Target = SharedWorker;

    fn deref(&self) -> &Self::Target {
        &self.worker
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.server_shutdown_tx.send(()).ok();
        let Some(reports) = self.worker.lock().unwrap().reports() else {
            return;
        };

        for report in reports {
            print_report(report);
        }

        panic!("assertion http request")
    }
}

impl Inner {
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

#[derive(Clone)]
struct InnerGuard {
    inner: Arc<Inner>,
    id: ExpectationId,
}

impl Deref for InnerGuard {
    type Target = SharedWorker;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl InnerGuard {
    fn new(inner: Arc<Inner>) -> Self {
        let id = inner.worker.lock().unwrap().create_next();
        Self { inner, id }
    }

    fn addr(&self) -> SocketAddr {
        self.inner.addr()
    }
}

/// HTTP mock server for use in tests.
///
/// Start with [`Whyhttp::run`], configure expectations via the builder chain,
/// then point your HTTP client at [`url`](Whyhttp::url).
///
/// On drop, any unfulfilled or violated expectations are printed and the
/// the test panics on drop - so a failing mock automatically fails the test.
///
/// See the [crate-level documentation](crate) for a full example.
pub struct Whyhttp {
    inner: Arc<Inner>,
}

/// Builder for routing matchers.
///
/// Obtained from [`Whyhttp::when`]. Chain matcher methods to narrow which
/// requests this expectation handles, then call [`response`](WhenWhyhttpRequest::response)
/// to configure the reply.
///
/// Routing matchers are evaluated on every incoming request; the first
/// expectation whose matchers all pass handles the request.
pub struct WhenWhyhttpRequest {
    inner: InnerGuard,
}

/// Builder for validating matchers.
///
/// Obtained from [`WhenWhyhttpRequest::should`]. Validating matchers run
/// after an expectation is selected by routing. They never affect which
/// response is returned - failures are collected and reported on drop.
///
/// Use this to assert *how* a matched request looks (e.g. confirm a specific
/// header is present) without affecting routing.
pub struct ShouldWhyhttpRequest {
    inner: InnerGuard,
}

/// Builder for configuring the mock response.
///
/// Obtained from [`WhenWhyhttpRequest::response`] or
/// [`ShouldWhyhttpRequest::response`]. Chain [`status`](WhyhttpResponse::status),
/// [`header`](WhyhttpResponse::header), and [`body`](WhyhttpResponse::body) to
/// build the response returned when this expectation is matched.
///
/// Calling [`when`](WhyhttpResponse::when) starts a **new** expectation,
/// which is useful for registering multiple mocks in one chain.
pub struct WhyhttpResponse {
    inner: InnerGuard,
}

impl Whyhttp {
    /// Starts the mock server on an ephemeral port.
    ///
    /// The server runs in a background thread and shuts down automatically
    /// when this instance is dropped. Unfulfilled expectations cause a panic
    /// on drop.
    ///
    /// ```
    /// let server = whyhttp::Whyhttp::run();
    /// assert!(server.url().starts_with("http://"));
    /// ```
    pub fn run() -> Self {
        let worker = SharedWorker::default();
        let (server_shutdown_tx, addr) = run_server(worker.clone());
        Self {
            inner: Arc::new(Inner {
                worker,
                addr,
                server_shutdown_tx,
            }),
        }
    }

    /// Creates a new expectation and enters the routing phase.
    ///
    /// The returned [`WhenWhyhttpRequest`] lets you add routing matchers.
    /// Call [`response`](WhenWhyhttpRequest::response) to configure the reply,
    /// or [`should`](WhenWhyhttpRequest::should) to add validating matchers first.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/api").method("POST").response().status(201);
    /// ```
    pub fn when(&self) -> WhenWhyhttpRequest {
        WhenWhyhttpRequest {
            inner: InnerGuard::new(self.inner.clone()),
        }
    }

    /// Creates a new expectation and enters the validation phase.
    ///
    /// Shorthand for `self.when().should()`. The expectation has no routing
    /// matchers, so it matches every request.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// // Assert every request carries this header, regardless of path.
    /// server.should().header("x-request-id", "abc").response().status(200);
    /// ```
    pub fn should(&self) -> ShouldWhyhttpRequest {
        self.when().should()
    }

    /// Creates a new expectation and enters the response phase.
    ///
    /// Shorthand for `self.when().response()`. No routing or validating
    /// matchers: the expectation matches every request.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.response().status(503).body("unavailable");
    /// ```
    pub fn response(&self) -> WhyhttpResponse {
        self.when().response()
    }
}

impl WhenWhyhttpRequest {
    /// Transitions to the validation phase for this expectation.
    ///
    /// Routing matchers added so far are preserved. Subsequent matcher calls
    /// add validating matchers that are checked after routing.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/api")
    ///     .should().method("POST").header("content-type", "application/json")
    ///     .response().status(200);
    /// ```
    pub fn should(&self) -> ShouldWhyhttpRequest {
        ShouldWhyhttpRequest {
            inner: self.inner.clone(),
        }
    }

    /// Transitions to the response phase for this expectation.
    ///
    /// Shorthand for `self.should().response()`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/ping").response().status(200);
    /// ```
    pub fn response(&self) -> WhyhttpResponse {
        self.should().response()
    }

    /// Routes requests where the path equals `path`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/api/users").response().status(200);
    /// ```
    pub fn path<S: Into<String>>(self, path: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::Path(path.into()));

        self
    }

    /// Routes requests where the HTTP method equals `method` (case-insensitive).
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().method("DELETE").response().status(204);
    /// ```
    pub fn method<S: Into<String>>(self, method: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::Method(method.into()));

        self
    }

    /// Routes requests where query parameter `key` equals `value`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().query("page", "2").response().status(200);
    /// ```
    pub fn query<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryEq(key.into(), value.into()));

        self
    }

    /// Routes requests where query parameter `key` is present (any value).
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().query_exists("token").response().status(200);
    /// ```
    pub fn query_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryExists(key.into()));

        self
    }

    /// Routes requests where query parameter `key` is absent.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().without_query("debug").response().status(200);
    /// ```
    pub fn without_query<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryMiss(key.into()));

        self
    }

    /// Routes requests where header `key` equals `value`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().header("accept", "application/json").response().status(200);
    /// ```
    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderEq(key.into(), value.into()));

        self
    }

    /// Routes requests where header `key` is present (any value).
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().header_exists("authorization").response().status(200);
    /// ```
    pub fn header_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderExists(key.into()));

        self
    }

    /// Routes requests where header `key` is absent.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().without_header("x-internal").response().status(200);
    /// ```
    pub fn without_header<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderMiss(key.into()));

        self
    }

    /// Routes requests where the body equals `body`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().body(r#"{"ok":true}"#).response().status(200);
    /// ```
    pub fn body<S: Into<String>>(self, body: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::BodyEq(body.into()));

        self
    }

    /// Routes requests with an empty body.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().without_body().response().status(400);
    /// ```
    pub fn without_body(self) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::BodyMiss);

        self
    }
}

impl ShouldWhyhttpRequest {
    /// Transitions to the response phase for this expectation.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/api").should().method("GET").response().status(200);
    /// ```
    pub fn response(&self) -> WhyhttpResponse {
        WhyhttpResponse {
            inner: self.inner.clone(),
        }
    }

    /// Creates a **new** expectation and enters the routing phase.
    ///
    /// Shorthand for `self.response().when()`. Use this to chain multiple
    /// expectation setups without storing intermediate values.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server
    ///     .when().path("/a").should().method("GET").response().status(200)
    ///     .when().path("/b").should().method("POST").response().status(201);
    /// ```
    pub fn when(&self) -> WhenWhyhttpRequest {
        self.response().when()
    }

    /// Validates that the path equals `path`.
    pub fn path<S: Into<String>>(self, path: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::Path(path.into()));

        self
    }

    /// Validates that the HTTP method equals `method` (case-insensitive).
    pub fn method<S: Into<String>>(self, method: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::Method(method.into()));

        self
    }

    /// Validates that query parameter `key` equals `value`.
    pub fn query<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryEq(key.into(), value.into()));

        self
    }

    /// Validates that query parameter `key` is present.
    pub fn query_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryExists(key.into()));

        self
    }

    /// Validates that query parameter `key` is absent.
    pub fn without_query<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryMiss(key.into()));

        self
    }

    /// Validates that header `key` equals `value`.
    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderEq(key.into(), value.into()));

        self
    }

    /// Validates that header `key` is present.
    pub fn header_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderExists(key.into()));

        self
    }

    /// Validates that header `key` is absent.
    pub fn without_header<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderMiss(key.into()));

        self
    }

    /// Validates that the body equals `body`.
    pub fn body<S: Into<String>>(self, body: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::BodyEq(body.into()));

        self
    }

    /// Validates that the body is empty.
    pub fn without_body(self) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::BodyMiss);

        self
    }

    /// Asserts this expectation is called exactly `times` times.
    ///
    /// A mismatch is reported on drop.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/api").should().times(3).response().status(200);
    /// ```
    pub fn times(self, times: u16) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_times(&self.inner.id, times);
        self
    }
}

impl WhyhttpResponse {
    /// Creates a **new** expectation and enters the routing phase.
    ///
    /// Use this to register multiple expectations in a single chain.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server
    ///     .when().path("/a").response().status(200)
    ///     .when().path("/b").response().status(201);
    /// ```
    pub fn when(&self) -> WhenWhyhttpRequest {
        WhenWhyhttpRequest {
            inner: InnerGuard::new(self.inner.inner.clone()),
        }
    }

    /// Sets the response status code. Defaults to `200`.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/created").response().status(201);
    /// ```
    pub fn status(self, status: u16) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_response_status(&self.inner.id, status);
        self
    }

    /// Adds a response header.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/data").response()
    ///     .header("content-type", "application/json");
    /// ```
    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_response_header(&self.inner.id, key.into(), value.into());
        self
    }

    /// Sets the response body.
    ///
    /// ```no_run
    /// let server = whyhttp::Whyhttp::run();
    /// server.when().path("/hello").response().body("world");
    /// ```
    pub fn body<S: Into<String>>(self, body: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_response_body(&self.inner.id, body.into());
        self
    }
}

macro_rules! impl_helpers {
    (url => $($base:ident),+) => {
        $(
            impl $base {
                /// Returns the server's socket address.
                pub fn addr(&self) -> SocketAddr {
                    self.inner.addr()
                }

                /// Returns the server's base URL (e.g. `http://127.0.0.1:PORT`).
                pub fn url(&self) -> String {
                    format!("http://{}", self.addr())
                }
            }
        )+
    };
    (when => $($base:ident),+) => {
        $(
            impl $base {
                /// Shorthand for `self.when().path(path)`.
                pub fn when_path<S: Into<String>>(&self, path: S) -> WhenWhyhttpRequest {
                    self.when().path(path)
                }

                /// Shorthand for `self.when().method(method)`.
                pub fn when_method<S: Into<String>>(&self, method: S) -> WhenWhyhttpRequest {
                    self.when().method(method)
                }

                /// Shorthand for `self.when().query(key, value)`.
                pub fn when_query<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhenWhyhttpRequest {
                    self.when().query(key, value)
                }

                /// Shorthand for `self.when().query_exists(key)`.
                pub fn when_query_exists<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().query_exists(key)
                }

                /// Shorthand for `self.when().without_query(key)`.
                pub fn when_without_query<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().without_query(key)
                }

                /// Shorthand for `self.when().header(key, value)`.
                pub fn when_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhenWhyhttpRequest {
                    self.when().header(key, value)
                }

                /// Shorthand for `self.when().header_exists(key)`.
                pub fn when_header_exists<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().header_exists(key)
                }

                /// Shorthand for `self.when().without_header(key)`.
                pub fn when_without_header<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().without_header(key)
                }

                /// Shorthand for `self.when().body(body)`.
                pub fn when_body<S: Into<String>>(&self, body: S) -> WhenWhyhttpRequest {
                    self.when().body(body)
                }

                /// Shorthand for `self.when().without_body()`.
                pub fn when_without_body(&self) -> WhenWhyhttpRequest {
                    self.when().without_body()
                }
            }
        )+
    };
    (should => $($base:ident),+) => {
        $(
            impl $base {
                /// Shorthand for `self.should().path(path)`.
                pub fn should_path<S: Into<String>>(&self, path: S) -> ShouldWhyhttpRequest {
                    self.should().path(path)
                }

                /// Shorthand for `self.should().method(method)`.
                pub fn should_method<S: Into<String>>(&self, method: S) -> ShouldWhyhttpRequest {
                    self.should().method(method)
                }

                /// Shorthand for `self.should().query(key, value)`.
                pub fn should_query<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> ShouldWhyhttpRequest {
                    self.should().query(key, value)
                }

                /// Shorthand for `self.should().query_exists(key)`.
                pub fn should_query_exists<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().query_exists(key)
                }

                /// Shorthand for `self.should().without_query(key)`.
                pub fn should_without_query<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().without_query(key)
                }

                /// Shorthand for `self.should().header(key, value)`.
                pub fn should_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> ShouldWhyhttpRequest {
                    self.should().header(key, value)
                }

                /// Shorthand for `self.should().header_exists(key)`.
                pub fn should_header_exists<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().header_exists(key)
                }

                /// Shorthand for `self.should().without_header(key)`.
                pub fn should_without_header<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().without_header(key)
                }

                /// Shorthand for `self.should().body(body)`.
                pub fn should_body<S: Into<String>>(&self, body: S) -> ShouldWhyhttpRequest {
                    self.should().body(body)
                }

                /// Shorthand for `self.should().without_body()`.
                pub fn should_without_body(&self) -> ShouldWhyhttpRequest {
                    self.should().without_body()
                }

                /// Shorthand for `self.should().times(times)`.
                pub fn should_times(&self, times: u16) -> ShouldWhyhttpRequest {
                    self.should().times(times)
                }
            }
        )+
    };
    (response => $($base:ident),+) => {
        $(
            impl $base {
                /// Shorthand for `self.response().status(status)`.
                pub fn response_status(&self, status: u16) -> WhyhttpResponse {
                    self.response().status(status)
                }

                /// Shorthand for `self.response().header(key, value)`.
                pub fn response_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhyhttpResponse {
                    self.response().header(key, value)
                }

                /// Shorthand for `self.response().body(body)`.
                pub fn response_body<S: Into<String>>(&self, body: S) -> WhyhttpResponse {
                    self.response().body(body)
                }
            }
        )+
    };
}

impl_helpers!(url => Whyhttp, WhenWhyhttpRequest, ShouldWhyhttpRequest, WhyhttpResponse);
impl_helpers!(when => Whyhttp, ShouldWhyhttpRequest, WhyhttpResponse);
impl_helpers!(should => Whyhttp, WhenWhyhttpRequest);
impl_helpers!(response => Whyhttp, WhenWhyhttpRequest, ShouldWhyhttpRequest);
