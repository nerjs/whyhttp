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

pub struct Whyhttp {
    inner: Arc<Inner>,
}

pub struct WhenWhyhttpRequest {
    inner: InnerGuard,
}

pub struct ShouldWhyhttpRequest {
    inner: InnerGuard,
}

pub struct WhyhttpResponse {
    inner: InnerGuard,
}

impl Whyhttp {
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

    pub fn when(&self) -> WhenWhyhttpRequest {
        WhenWhyhttpRequest {
            inner: InnerGuard::new(self.inner.clone()),
        }
    }

    pub fn should(&self) -> ShouldWhyhttpRequest {
        self.when().should()
    }

    pub fn response(&self) -> WhyhttpResponse {
        self.when().response()
    }
}

impl WhenWhyhttpRequest {
    pub fn should(&self) -> ShouldWhyhttpRequest {
        ShouldWhyhttpRequest {
            inner: self.inner.clone(),
        }
    }

    pub fn response(&self) -> WhyhttpResponse {
        self.should().response()
    }

    pub fn path<S: Into<String>>(self, path: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::Path(path.into()));

        self
    }

    pub fn method<S: Into<String>>(self, method: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::Method(method.into()));

        self
    }

    pub fn query<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryEq(key.into(), value.into()));

        self
    }

    pub fn query_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryExists(key.into()));

        self
    }
    pub fn without_query<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::QueryMiss(key.into()));

        self
    }
    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderEq(key.into(), value.into()));

        self
    }

    pub fn header_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderExists(key.into()));

        self
    }
    pub fn without_header<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::HeaderMiss(key.into()));

        self
    }
    pub fn body<S: Into<String>>(self, body: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::BodyEq(body.into()));

        self
    }
    pub fn without_body(self) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_routing(&self.inner.id, Matcher::BodyMiss);

        self
    }
}

impl ShouldWhyhttpRequest {
    pub fn response(&self) -> WhyhttpResponse {
        WhyhttpResponse {
            inner: self.inner.clone(),
        }
    }

    pub fn when(&self) -> WhenWhyhttpRequest {
        self.response().when()
    }

    pub fn path<S: Into<String>>(self, path: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::Path(path.into()));

        self
    }

    pub fn method<S: Into<String>>(self, method: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::Method(method.into()));

        self
    }

    pub fn query<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryEq(key.into(), value.into()));

        self
    }

    pub fn query_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryExists(key.into()));

        self
    }
    pub fn without_query<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::QueryMiss(key.into()));

        self
    }
    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderEq(key.into(), value.into()));

        self
    }

    pub fn header_exists<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderExists(key.into()));

        self
    }
    pub fn without_header<K: Into<String>>(self, key: K) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::HeaderMiss(key.into()));

        self
    }
    pub fn body<S: Into<String>>(self, body: S) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::BodyEq(body.into()));

        self
    }
    pub fn without_body(self) -> Self {
        self.inner
            .lock()
            .unwrap()
            .add_validating(&self.inner.id, Matcher::BodyMiss);

        self
    }

    pub fn times<T: Into<u16>>(self, times: T) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_times(&self.inner.id, times.into());
        self
    }
}

impl WhyhttpResponse {
    pub fn when(&self) -> WhenWhyhttpRequest {
        WhenWhyhttpRequest {
            inner: InnerGuard::new(self.inner.inner.clone()),
        }
    }

    pub fn status<N: Into<u16>>(self, status: N) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_response_status(&self.inner.id, status.into());
        self
    }

    pub fn header<K: Into<String>, V: Into<String>>(self, key: K, value: V) -> Self {
        self.inner
            .lock()
            .unwrap()
            .set_response_header(&self.inner.id, key.into(), value.into());
        self
    }

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
                pub fn addr(&self) -> SocketAddr {
                    self.inner.addr()
                }

                pub fn url(&self) -> String {
                    format!("http://{}", self.addr())
                }
            }
        )+
    };
    (when => $($base:ident),+) => {
        $(
            impl $base {
                pub fn when_path<S: Into<String>>(&self, path: S) -> WhenWhyhttpRequest {
                    self.when().path(path)
                }

                pub fn when_method<S: Into<String>>(&self, method: S) -> WhenWhyhttpRequest {
                    self.when().method(method)
                }

                pub fn when_query<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhenWhyhttpRequest {
                    self.when().query(key, value)
                }

                pub fn when_query_exists<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().query_exists(key)
                }

                pub fn when_without_query<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().without_query(key)
                }

                pub fn when_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhenWhyhttpRequest {
                    self.when().header(key, value)
                }

                pub fn when_header_exists<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().header_exists(key)
                }

                pub fn when_without_header<K: Into<String>>(&self, key: K) -> WhenWhyhttpRequest {
                    self.when().without_header(key)
                }

                pub fn when_body<S: Into<String>>(&self, body: S) -> WhenWhyhttpRequest {
                    self.when().body(body)
                }

                pub fn when_without_body(&self) -> WhenWhyhttpRequest {
                    self.when().without_body()
                }
            }
        )+
    };
    (should => $($base:ident),+) => {
        $(
            impl $base {

                pub fn should_path<S: Into<String>>(&self, path: S) -> ShouldWhyhttpRequest {
                    self.should().path(path)
                }

                pub fn should_method<S: Into<String>>(&self, method: S) -> ShouldWhyhttpRequest {
                    self.should().method(method)
                }

                pub fn should_query<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> ShouldWhyhttpRequest {
                    self.should().query(key, value)
                }

                pub fn should_query_exists<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().query_exists(key)
                }

                pub fn should_without_query<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().without_query(key)
                }

                pub fn should_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> ShouldWhyhttpRequest {
                    self.should().header(key, value)
                }

                pub fn should_header_exists<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().header_exists(key)
                }

                pub fn should_without_header<K: Into<String>>(&self, key: K) -> ShouldWhyhttpRequest {
                    self.should().without_header(key)
                }

                pub fn should_body<S: Into<String>>(&self, body: S) -> ShouldWhyhttpRequest {
                    self.should().body(body)
                }

                pub fn should_without_body(&self) -> ShouldWhyhttpRequest {
                    self.should().without_body()
                }

                pub fn should_times<T: Into<u16>>(&self, times: T) -> ShouldWhyhttpRequest {
                    self.should().times(times)
                }
            }
        )+
    };
    (response => $($base:ident),+) => {
        $(
            impl $base {

                pub fn response_status<N: Into<u16>>(&self, status: N) -> WhyhttpResponse {
                    self.response().status(status)
                }

                pub fn response_header<K: Into<String>, V: Into<String>>(&self, key: K, value: V) -> WhyhttpResponse {
                    self.response().header(key, value)
                }

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
