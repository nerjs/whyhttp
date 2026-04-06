//! Integration tests for the fluent API: url/addr helpers, builder chains,
//! and Whyhttp-level shorthands (should/response).

use reqwest::blocking::Client;
use whyhttp::Whyhttp;

fn client() -> Client {
    Client::new()
}

#[test]
fn url_and_addr_on_whyhttp() {
    let server = Whyhttp::run();
    server.response(); // registers catch-all expectation

    assert!(server.url().starts_with("http://"));
    let addr = server.addr();
    assert_ne!(addr.port(), 0);

    client().get(server.url()).send().unwrap();
}

#[test]
fn url_accessible_from_all_builder_stages() {
    // url() is available on WhenWhyhttpRequest, ShouldWhyhttpRequest, WhyhttpResponse
    let server = Whyhttp::run();

    let when = server.when().path("/test");
    assert!(when.url().starts_with("http://"));

    let should = when.should();
    assert!(should.url().starts_with("http://"));

    let resp_builder = should.response().status(200u16);
    assert!(resp_builder.url().starts_with("http://"));

    client()
        .get(format!("{}/test", server.url()))
        .send()
        .unwrap();
}

#[test]
fn addr_accessible_from_all_builder_stages() {
    let server = Whyhttp::run();

    let when = server.when().path("/check");
    assert_ne!(when.addr().port(), 0);

    let should = when.should();
    assert_ne!(should.addr().port(), 0);

    let resp_builder = should.response().status(200u16);
    assert_ne!(resp_builder.addr().port(), 0);

    client()
        .get(format!("{}/check", server.url()))
        .send()
        .unwrap();
}

#[test]
fn whyhttp_should_shorthand() {
    // Whyhttp::should() is shorthand for when().should()
    // Creates an expectation with no routing matchers
    let server = Whyhttp::run();
    server
        .should()
        .header("x-token", "abc")
        .response()
        .status(200u16);

    let resp = client()
        .get(server.url())
        .header("x-token", "abc")
        .send()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

#[test]
fn whyhttp_response_shorthand() {
    // Whyhttp::response() is shorthand for when().response()
    let server = Whyhttp::run();
    server.response().status(204u16);

    let resp = client().get(server.url()).send().unwrap();
    assert_eq!(resp.status().as_u16(), 204);
}

#[test]
fn chain_via_response_when() {
    // WhyhttpResponse::when() starts a new expectation — enables fluent multi-mock setup
    let server = Whyhttp::run();
    server
        .when()
        .path("/users")
        .response()
        .status(200u16)
        .when()
        .path("/orders")
        .response()
        .status(201u16);

    let r1 = client()
        .get(format!("{}/users", server.url()))
        .send()
        .unwrap();
    assert_eq!(r1.status().as_u16(), 200);

    let r2 = client()
        .get(format!("{}/orders", server.url()))
        .send()
        .unwrap();
    assert_eq!(r2.status().as_u16(), 201);
}

#[test]
fn chain_via_should_when() {
    // ShouldWhyhttpRequest::when() also starts a new expectation
    let server = Whyhttp::run();
    server
        .when()
        .path("/a")
        .should()
        .method("GET")
        .response()
        .status(200u16)
        .when()
        .path("/b")
        .should()
        .method("POST")
        .response()
        .status(201u16);

    let r1 = client()
        .get(format!("{}/a", server.url()))
        .send()
        .unwrap();
    assert_eq!(r1.status().as_u16(), 200);

    let r2 = client()
        .post(format!("{}/b", server.url()))
        .send()
        .unwrap();
    assert_eq!(r2.status().as_u16(), 201);
}
