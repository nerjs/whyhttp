//! Integration tests for should() validating matchers.
//! Validating matchers run after routing — they never block the response,
//! but failures are reported on drop. All tests here send requests that
//! satisfy the should conditions, so the server drops cleanly.

use reqwest::blocking::Client;
use whyhttp::Whyhttp;

fn client() -> Client {
    Client::new()
}

#[test]
fn should_path() {
    let server = Whyhttp::run();
    // no routing matchers → matches all; should validates path
    server.when().should().path("/api").response().status(200u16);

    client()
        .get(format!("{}/api", server.url()))
        .send()
        .unwrap();
}

#[test]
fn should_method() {
    let server = Whyhttp::run();
    server.when().should().method("GET").response().status(200u16);

    client().get(server.url()).send().unwrap();
}

#[test]
fn should_query() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .query("page", "1")
        .response()
        .status(200u16);

    client()
        .get(format!("{}/?page=1", server.url()))
        .send()
        .unwrap();
}

#[test]
fn should_query_exists() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .query_exists("token")
        .response()
        .status(200u16);

    client()
        .get(format!("{}/?token=xyz", server.url()))
        .send()
        .unwrap();
}

#[test]
fn should_without_query() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .without_query("debug")
        .response()
        .status(200u16);

    client().get(server.url()).send().unwrap();
}

#[test]
fn should_header() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .header("x-token", "secret")
        .response()
        .status(200u16);

    client()
        .get(server.url())
        .header("x-token", "secret")
        .send()
        .unwrap();
}

#[test]
fn should_header_exists() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .header_exists("x-token")
        .response()
        .status(200u16);

    client()
        .get(server.url())
        .header("x-token", "any")
        .send()
        .unwrap();
}

#[test]
fn should_without_header() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .without_header("x-internal")
        .response()
        .status(200u16);

    client().get(server.url()).send().unwrap();
}

#[test]
fn should_body() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .body("payload")
        .response()
        .status(200u16);

    // POST is used only because reqwest requires a body-capable method; the method does not affect routing or validation.
    client()
        .post(server.url())
        .body("payload")
        .send()
        .unwrap();
}

#[test]
fn should_without_body() {
    let server = Whyhttp::run();
    server
        .when()
        .should()
        .without_body()
        .response()
        .status(200u16);

    client().get(server.url()).send().unwrap();
}

#[test]
fn should_times_exact() {
    // expectation must be called exactly N times
    let server = Whyhttp::run();
    server
        .when()
        .path("/ping")
        .should()
        .times(2u16)
        .response()
        .status(200u16);

    client()
        .get(format!("{}/ping", server.url()))
        .send()
        .unwrap();
    client()
        .get(format!("{}/ping", server.url()))
        .send()
        .unwrap();
    // exactly 2 calls → clean drop
}
