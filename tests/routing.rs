//! Integration tests for `when()` routing matchers.

use reqwest::blocking::Client;
use whyhttp::Whyhttp;

fn client() -> Client {
    Client::new()
}

#[test]
fn route_by_path() {
    let server = Whyhttp::run();
    server.when().path("/hello").response().status(201);

    let resp = client()
        .get(format!("{}/hello", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_method() {
    let server = Whyhttp::run();
    server.when().method("POST").response().status(201);

    let resp = client().post(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_method_case_insensitive() {
    // method matching ignores ASCII case
    let server = Whyhttp::run();
    server.when().method("get").response().status(201);

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_query() {
    let server = Whyhttp::run();
    server.when().query("page", "2").response().status(201);

    let resp = client()
        .get(format!("{}/?page=2", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_query_exists() {
    let server = Whyhttp::run();
    server.when().query_exists("token").response().status(201);

    let resp = client()
        .get(format!("{}/?token=abc", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_without_query() {
    let server = Whyhttp::run();
    server.when().without_query("debug").response().status(201);

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_header() {
    let server = Whyhttp::run();
    server
        .when()
        .header("x-api-key", "secret")
        .response()
        .status(201);

    let resp = client()
        .get(server.url())
        .header("x-api-key", "secret")
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_header_exists() {
    let server = Whyhttp::run();
    server
        .when()
        .header_exists("authorization")
        .response()
        .status(201);

    let resp = client()
        .get(server.url())
        .header("authorization", "Bearer token")
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_without_header() {
    let server = Whyhttp::run();
    server
        .when()
        .without_header("x-internal")
        .response()
        .status(201);

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_body() {
    let server = Whyhttp::run();
    server.when().body(r#"{"ok":true}"#).response().status(201);

    let resp = client()
        .post(server.url())
        .body(r#"{"ok":true}"#)
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_without_body() {
    let server = Whyhttp::run();
    server.when().without_body().response().status(201);

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 201);
}

#[test]
fn route_by_combined_matchers() {
    // all matchers on an expectation must pass for it to be selected
    let server = Whyhttp::run();
    server
        .when()
        .path("/api")
        .method("POST")
        .response()
        .status(201);
    // fallback: path only (catches GET /api)
    server.when().path("/api").response().status(200);

    let post = client()
        .post(format!("{}/api", server.url()))
        .send()
        .unwrap();
    assert_eq!(post.status().as_u16(), 201);

    let get = client()
        .get(format!("{}/api", server.url()))
        .send()
        .unwrap();
    assert_eq!(get.status().as_u16(), 200);
}

#[test]
fn first_match_wins() {
    let server = Whyhttp::run();
    server.when().path("/item").response().status(200); // wins for /item
    server.when().response().status(500); // catch-all, lower priority

    let resp = client()
        .get(format!("{}/item", server.url()))
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    // satisfy the catch-all expectation with a different path
    client()
        .get(format!("{}/other", server.url()))
        .send()
        .unwrap();
}
