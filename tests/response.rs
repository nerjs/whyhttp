//! Integration tests for response configuration.
//! Shows how to set status, headers, and body on matched responses.

use reqwest::blocking::Client;
use whyhttp::Whyhttp;

fn client() -> Client {
    Client::new()
}

#[test]
fn default_response_is_200_empty_body() {
    // without explicit configuration the server returns 200 with no body
    let server = Whyhttp::run();
    server.response(); // registers a catch-all expectation with default config

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().unwrap(), "");
}

#[test]
fn response_status() {
    let server = Whyhttp::run();
    server.when().path("/teapot").response().status(418u16);

    let resp = client()
        .get(format!("{}/teapot", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 418);
}

#[test]
fn response_header() {
    let server = Whyhttp::run();
    server
        .response()
        .header("content-type", "application/json");

    let resp = client().get(server.url()).send().unwrap();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok());
    assert_eq!(ct, Some("application/json"));
}

#[test]
fn response_multiple_headers() {
    let server = Whyhttp::run();
    server
        .response()
        .header("x-request-id", "abc123")
        .header("cache-control", "no-cache");

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(
        resp.headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok()),
        Some("abc123")
    );
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );
}

#[test]
fn response_body() {
    let server = Whyhttp::run();
    server.response().body("hello world");

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.text().unwrap(), "hello world");
}

#[test]
fn response_full() {
    // status + header + body together
    let server = Whyhttp::run();
    server
        .when()
        .path("/data")
        .response()
        .status(201u16)
        .header("content-type", "application/json")
        .body(r#"{"id":1}"#);

    let resp = client()
        .get(format!("{}/data", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(resp.text().unwrap(), r#"{"id":1}"#);
}
