#![allow(unused)]
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub path: String,
    pub query: HashMap<String, Option<String>>,
    pub fragment: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            path: String::from("/"),
            query: Default::default(),
            fragment: Default::default(),
            headers: Default::default(),
            body: Default::default(),
        }
    }
}

fn split_str_by<'a>(input: &'a str, delimiter: &str) -> (&'a str, Option<&'a str>) {
    input
        .split_once(delimiter)
        .map(|(p, f)| (p, if f.is_empty() { None } else { Some(f) }))
        .unwrap_or((input, None))
}
impl From<&str> for Request {
    fn from(value: &str) -> Self {
        let (path, fragment) = split_str_by(value.trim().trim_start_matches("/"), "#");
        let (path, query) = split_str_by(path, "?");
        let mut request = Self {
            path: format!("/{path}"),
            fragment: fragment.map(String::from),
            ..Default::default()
        };

        if let Some(query) = query {
            request.query = query
                .split("&")
                .map(|s| split_str_by(s, "="))
                .map(|(k, v)| (k.to_string(), v.map(String::from)))
                .collect();
        }

        request
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[rstest::rstest]
    #[case("", Request::default())]
    #[case("/", Request::default())]
    #[case("/some/path", Request { path: "/some/path".into(), ..Default::default() })]
    #[case("/path?key=value", Request { path: "/path".into(), query: [("key".into(), Some("value".into()))].into(), ..Default::default() })]
    #[case("/path?key=value#some-hash", Request { path: "/path".into(), query: [("key".into(), Some("value".into()))].into(), fragment: Some("some-hash".into()), ..Default::default() })]
    #[case("?key=value&empty_key", Request { query: [("key".into(), Some("value".into())), ("empty_key".into(), None)].into(), ..Default::default() })]
    fn from_str(#[case] uri: &str, #[case] request: Request) {
        assert_eq!(
            Request::from(uri),
            request,
            "The request with {uri:?} should be parsed into {request:?}"
        );
    }
}
