# whyhttp

HTTP mock server for use in tests. Starts a real server in a background thread,
expectations are configured via a fluent builder API and verified automatically on drop.

## Quick start

```rust
let server = whyhttp::Whyhttp::run();

server
    .when().path("/greet").method("GET")
    .should().header("x-token", "secret")
    .response().status(200).body("hello");

let resp = reqwest::blocking::Client::new()
    .get(format!("{}/greet", server.url()))
    .header("x-token", "secret")
    .send()
    .unwrap();

assert_eq!(resp.status().as_u16(), 200);
assert_eq!(resp.text().unwrap(), "hello");
```

## Routing vs. validation

Every expectation has two independent sets of matchers:

- **`when`**: *routing* matchers: select which expectation handles the request.
  The first expectation whose matchers all pass wins.
- **`should`**: *validating* matchers: run after routing. They never affect
  which response is returned - failures appear in the drop report.

```rust,no_run
let server = whyhttp::Whyhttp::run();

// Route only POST /orders, but assert the body separately.
server
    .when().path("/orders").method("POST")
    .should().body(r#"{"qty":1}"#)
    .response().status(201);
```

## Drop behavior

When `Whyhttp` is dropped, all collected issues are printed and the test panics
if anything is wrong - so a failing mock automatically fails the test. Possible issues:

| Report | Cause |
|---|---|
| `NoCall` | Expectation was configured but never triggered |
| `MismatchTimes` | Expectation was triggered the wrong number of times |
| `UnmatchedRequest` | Request arrived but no expectation matched it |
| `Matcher` | Routing matched, but a `should` matcher failed |

## Matchers

Both `when` (routing) and `should` (validating) support the same set:

| Method | Matches when... |
|---|---|
| `path(p)` | path equals `p` |
| `method(m)` | HTTP method equals `m` (case-insensitive) |
| `query(k, v)` | query param `k` equals `v` |
| `query_exists(k)` | query param `k` is present |
| `without_query(k)` | query param `k` is absent |
| `header(k, v)` | header `k` equals `v` |
| `header_exists(k)` | header `k` is present |
| `without_header(k)` | header `k` is absent |
| `body(b)` | body equals `b` |
| `without_body()` | body is empty |

## Call count assertion

```rust,no_run
let server = whyhttp::Whyhttp::run();

// This endpoint must be called exactly twice.
server.when().path("/api").should().times(2).response().status(200);
```
