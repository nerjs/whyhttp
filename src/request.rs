use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, Option<String>>,
    pub fragment: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Request {
    pub fn set_path<S: Into<String>>(&mut self, path: S) {
        self.path = path.into();
    }

    pub fn set_method<S: Into<String>>(&mut self, method: S) {
        self.method = method.into();
    }

    pub fn set_fragment<S: Into<String>>(&mut self, fragment: S) {
        self.fragment = Some(fragment.into());
    }

    pub fn set_body<S: Into<String>>(&mut self, body: S) {
        self.body = Some(body.into());
    }

    pub fn set_query<K: Into<String>, V: Into<String>>(&mut self, key: K, value: Option<V>) {
        self.query.insert(key.into(), value.map(|s| s.into()));
    }

    pub fn set_header<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.headers.insert(key.into(), value.into());
    }

    pub fn with_path<S: Into<String>>(mut self, path: S) -> Self {
        self.set_path(path);
        self
    }

    pub fn with_method<S: Into<String>>(mut self, method: S) -> Self {
        self.set_method(method);
        self
    }

    pub fn with_fragment<S: Into<String>>(mut self, fragment: S) -> Self {
        self.set_fragment(fragment);
        self
    }

    pub fn with_body<S: Into<String>>(mut self, body: S) -> Self {
        self.set_body(body);
        self
    }

    pub fn with_query<K: Into<String>, V: Into<String>>(
        mut self,
        key: K,
        value: Option<V>,
    ) -> Self {
        self.set_query(key, value);
        self
    }

    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.set_header(key, value);
        self
    }
}

impl Default for Request {
    fn default() -> Self {
        Self {
            method: String::from("GET"),
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

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("[{} {}", self.method.to_uppercase(), self.path));

        if !self.query.is_empty() {
            f.write_str("?");
            let query = self
                .query
                .iter()
                .map(|(k, v)| {
                    if let Some(v) = v {
                        format!("{}={}", k, v)
                    } else {
                        k.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join("&");
            f.write_str(&query);
        }

        if let Some(fragment) = &self.fragment {
            f.write_str(&format!("#{fragment}"));
        }

        if !self.headers.is_empty() {
            f.write_str(" | with headers {");

            let headers = self
                .headers
                .iter()
                .map(|(k, v)| format!("{k:?} = {v:?}"))
                .collect::<Vec<String>>()
                .join(", ");
            f.write_str(&headers);

            f.write_str("}");
        }

        if let Some(body) = &self.body {
            f.write_str(&format!(" | with body {body:?}"));
        }

        f.write_str("]");

        Ok(())
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
