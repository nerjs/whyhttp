//! Integration tests for the drop-time assertion mechanism.
//!
//! When `Whyhttp` is dropped, it panics if any expectation was violated.
//! This is the primary way the library fails tests automatically.
//! Tests use `catch_unwind(AssertUnwindSafe(...))` to verify these panics.

use reqwest::blocking::Client;
use std::panic::AssertUnwindSafe;
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
};
use whyhttp::Whyhttp;

/// Assert that the closure causes a panic (i.e. the server found violations on drop).
fn must_panic(f: impl FnOnce()) {
    let err = std::panic::catch_unwind(AssertUnwindSafe(f))
        .expect_err("expected Whyhttp to panic on drop, but it did not");
    let msg = err
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| err.downcast_ref::<String>().map(String::as_str));
    assert_eq!(msg, Some("assertion http request"));
}

#[test]
fn no_call_panics() {
    // An expectation that is configured but never triggered causes a panic on drop.
    must_panic(|| {
        let server = Whyhttp::run();
        server.when().path("/api").response().status(200u16);
        // no request made
    });
}

#[test]
fn unmatched_request_panics() {
    // Requests with no matching expectation are recorded as errors and cause a panic on drop.
    must_panic(|| {
        let server = Whyhttp::run();
        // no expectations registered — every request is unmatched
        Client::new().get(server.url()).send().unwrap();
    });
}

#[test]
fn unmatched_request_returns_default_200() {
    // Even though the request is unmatched, the server still responds with 200.
    // The panic happens on drop, not during the response.
    let status_code = Arc::new(AtomicU16::new(0));
    let status_clone = status_code.clone();

    must_panic(move || {
        let server = Whyhttp::run();
        let resp = Client::new().get(server.url()).send().unwrap();
        status_clone.store(resp.status().as_u16(), Ordering::SeqCst);
    });

    assert_eq!(
        status_code.load(Ordering::SeqCst),
        200,
        "unmatched request should still return 200"
    );
}

#[test]
fn mismatch_times_panics() {
    // times(N) panics when triggered fewer times than expected.
    must_panic(|| {
        let server = Whyhttp::run();
        server
            .when()
            .path("/api")
            .should()
            .times(2u16)
            .response()
            .status(200u16);
        Client::new()
            .get(format!("{}/api", server.url()))
            .send()
            .unwrap();
        // called once instead of twice
    });
}

#[test]
fn mismatch_times_over_call_panics() {
    // times(N) panics when triggered more times than expected.
    must_panic(|| {
        let server = Whyhttp::run();
        server
            .when()
            .path("/api")
            .should()
            .times(1u16)
            .response()
            .status(200u16);
        let client = Client::new();
        client.get(format!("{}/api", server.url())).send().unwrap();
        client.get(format!("{}/api", server.url())).send().unwrap();
        // called twice instead of once
    });
}

#[test]
fn matcher_failure_panics() {
    // A should-matcher failure is reported on drop, even though the response was returned.
    must_panic(|| {
        let server = Whyhttp::run();
        // routes on path, validates method
        server
            .when()
            .path("/api")
            .should()
            .method("POST")
            .response()
            .status(200u16);
        // GET matches routing (path=/api) but fails should (method≠POST)
        Client::new()
            .get(format!("{}/api", server.url()))
            .send()
            .unwrap();
    });
}

#[test]
fn matcher_failure_still_returns_response() {
    // The should-matcher failure does not block the response — it is reported only on drop.
    let status_code = Arc::new(AtomicU16::new(0));
    let status_clone = status_code.clone();

    must_panic(move || {
        let server = Whyhttp::run();
        server
            .when()
            .path("/api")
            .should()
            .method("POST")
            .response()
            .status(200u16);

        let resp = Client::new()
            .get(format!("{}/api", server.url()))
            .send()
            .unwrap();
        status_clone.store(resp.status().as_u16(), Ordering::SeqCst);
    });

    assert_eq!(
        status_code.load(Ordering::SeqCst),
        200,
        "response must be returned even when should-matcher fails"
    );
}

#[test]
fn clean_drop_no_panic() {
    // All expectations met → clean drop.
    let server = Whyhttp::run();
    server.when().path("/ok").response().status(200u16);
    Client::new()
        .get(format!("{}/ok", server.url()))
        .send()
        .unwrap();
}
