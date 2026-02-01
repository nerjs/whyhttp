use crate::request::Request;

#[derive(Debug, Clone, PartialEq)]
pub enum Matcher {
    Method(String),
    Path(String),
    QueryExists(String),
    QueryMiss(String),
    QueryEq(String, String),
    FragmentEq(String),
    FragmentMiss,
    HeaderExists(String),
    HeaderMiss(String),
    HeaderEq(String, String),
    BodyMiss,
    BodyEq(String),
}

impl Matcher {
    pub fn validate(&self, request: &Request) -> Option<Matcher> {
        match self {
            Matcher::Method(expected) if request.method.to_uppercase() != expected.to_uppercase() => 
                    Some(Matcher::Method(request.method.clone())),
               
            Matcher::Path(expected) if &request.path != expected => Some(Matcher::Path(request.path.clone())),
            Matcher::QueryEq(key, expected_val) => {
                match request.query.get(key) {
                    Some(Some(actual_val)) if actual_val == expected_val => None,
                    Some(Some(actual_val)) => Some(Matcher::QueryEq(key.clone(), actual_val.clone())),
                    Some(None) => Some(Matcher::QueryExists(key.clone())),
                    None => Some(Matcher::QueryMiss(key.clone())),
                }
            }
            Matcher::QueryExists(key) if !request.query.contains_key(key) => Some(Matcher::QueryMiss(key.clone())),
            Matcher::QueryMiss(key) if request.query.contains_key(key) => Some(Matcher::QueryExists(key.clone())),
            Matcher::HeaderEq(key, expected_val) => {
                match request.headers.get(key) {
                    Some(actual_val) if actual_val == expected_val => None,
                    Some(actual_val) => Some(Matcher::HeaderEq(key.clone(), actual_val.clone())),
                    None => Some(Matcher::HeaderMiss(key.clone())),
                }
            }
            Matcher::HeaderExists(key) if !request.headers.contains_key(key) =>  Some(Matcher::HeaderMiss(key.clone())),
            Matcher::HeaderMiss(key) if request.headers.contains_key(key) => Some(Matcher::HeaderExists(key.clone())),
            Matcher::FragmentEq(expected) => {
                match &request.fragment {
                    Some(actual) if actual == expected => None,
                    Some(actual) => Some(Matcher::FragmentEq(actual.clone())),
                    None => Some(Matcher::FragmentMiss),
                }
            }
            Matcher::FragmentMiss if request.fragment.is_some() => Some(Matcher::FragmentEq(request.fragment.clone().unwrap())),
            Matcher::BodyEq(expected) => {
                match &request.body {
                    Some(actual) if actual == expected => None,
                    Some(actual) => Some(Matcher::BodyEq(actual.clone())),
                    None => Some(Matcher::BodyMiss),
                }
            }
            Matcher::BodyMiss if request.body.is_some() => Some(Matcher::BodyEq(request.body.clone().unwrap())),
            _ => None
        }
    }
}

pub struct Matchers {
    inner: Vec<Matcher>,
}

impl Matchers {
    pub fn add(&mut self, matcher: Matcher) {}

    pub fn is_matched(&self, request: &Request) -> bool {
        self.inner.iter().all(|matcher| matcher.validate(request).is_none())
    }

    pub fn validate(&self, request: &Request) -> Option<Vec<Matcher>> {
        let errors: Vec<Matcher> = self.inner
            .iter()
            .filter_map(|matcher| matcher.validate(request))
            .collect();
        
        if errors.is_empty() {
            None
        } else {
            Some(errors)
        }
    }
}

#[cfg(test)]
mod test {
    use super::Matcher::*;
    use super::*;
    use rstest::rstest;

    // Helper functions for creating Matcher variants.
    // Solve two problems:
    // 1. Reduce verbosity in tests - use method("GET") instead of Matcher::Method("GET".into())
    // 2. Automatically convert &str to String via .into(), eliminating boilerplate
    //
    // Example usage in tests:
    // #[case::method(method("POST"), method("GET"), ...)]  // instead of
    // #[case::method(Matcher::Method("POST".into()), Matcher::Method("GET".into()), ...)]

    fn method(method: &str) -> Matcher {
        Matcher::Method(method.into())
    }

    fn path(path: &str) -> Matcher {
        Matcher::Path(path.into())
    }

    fn q_eq(key: &str, val: &str) -> Matcher {
        Matcher::QueryEq(key.into(), val.into())
    }

    fn q_ex(key: &str) -> Matcher {
        Matcher::QueryExists(key.into())
    }

    fn q_miss(key: &str) -> Matcher {
        Matcher::QueryMiss(key.into())
    }

    fn h_eq(key: &str, val: &str) -> Matcher {
        Matcher::HeaderEq(key.into(), val.into())
    }

    fn h_ex(key: &str) -> Matcher {
        Matcher::HeaderExists(key.into())
    }

    fn h_miss(key: &str) -> Matcher {
        Matcher::HeaderMiss(key.into())
    }

    fn f_eq(fragment: &str) -> Matcher {
        Matcher::FragmentEq(fragment.into())
    }

    fn f_miss() -> Matcher {
        Matcher::FragmentMiss
    }

    fn b_eq(body: &str) -> Matcher {
        Matcher::BodyEq(body.into())
    }

    fn b_miss() -> Matcher {
        Matcher::BodyMiss
    }

    #[rstest]
    #[case::method(method("post"), method("GET"), Request::default())]
    #[case::method(method("PUT"), method("POST"), Request::default().with_method("POST"))]
    #[case::path(path("/invalid/path"), path("/some/path"), "/some/path".into())]
    #[case::path(path("/some"), path("/"), Request::default())]
    #[case::query(q_eq("q_key", "q2_val"), q_eq("q_key", "q_val"), "/?q_key=q_val".into())]
    #[case::query(q_miss("q_key"), q_ex("q_key"), "/?q_key=q_val".into())]
    #[case::query(q_ex("miss_key"), q_miss("miss_key"), "/?q_key=q_val".into())]
    #[case::query(q_eq("miss_key", "some_val"), q_miss("miss_key"), "/?q_key=q_val".into())]
    #[case::query(q_miss("exists_key"), q_ex("exists_key"), "/?q_key=q_val&exists_key".into())]
    #[case::fragment(f_eq("anchor-incorrect"), f_eq("anchor"), "/path#anchor".into())]
    #[case::fragment(f_miss(), f_eq("anchor"), "/path#anchor".into())]
    #[case::fragment(f_eq("anchor"), f_miss(), "/path".into())]
    #[case::header(h_eq("eq-header", "eq-incorrect-value"), h_eq("eq-header", "eq-value"), Request::default().with_header("eq-header", "eq-value"))]
    #[case::header(h_miss("eq-header"), h_ex("eq-header"), Request::default().with_header("eq-header", "eq-value"))]
    #[case::header(h_ex("miss-header"), h_miss("miss-header"), Request::default().with_header("eq-header", "eq-value"))]
    #[case::header(h_eq("miss-header", "some-miss-val"), h_miss("miss-header"), Request::default().with_header("eq-header", "eq-value"))]
    #[case::header(h_miss("exists-header"), h_ex("exists-header"), Request::default().with_header("exists-header", "some-exists-value"))]
    #[case::body(b_eq("some body"), b_miss(), Request::default())]
    #[case::body(b_eq("some incorrect body"), b_eq("some body"), Request::default().with_body("some body"))]
    #[case::body(b_miss(), b_eq("some body"), Request::default().with_body("some body"))]
    fn validate_once_matcher(
        #[case] invalid_matcher: Matcher,
        #[case] valid_matcher: Matcher,
        #[case] request: Request,
    ) {
        let report = invalid_matcher.validate(&request);
        assert_eq!(
            report, 
            Some(valid_matcher.clone()),
            "Invalid matcher {:?} should report expected correction {:?} for request: {}",
            invalid_matcher, valid_matcher, request
        );

        let result = valid_matcher.validate(&request);
        assert!(
            result.is_none(),
            "Valid matcher {:?} should pass validation (return None) for request: {}",
            valid_matcher, request
        );
    }

    #[rstest::rstest]
    #[case::empty(&[], Request::default())]
    #[case::method(&[method("GET")], Request::default())]
    #[case::method_path(&[method("POST"), path("/some/path")], Request::from("/some/path").with_method("POST"))]
    #[case::query(&[q_eq("key-eq", "val-eq")], "/?key-eq=val-eq".into())]
    #[case::query(&[q_ex("key-exists")], "/?key-exists".into())]
    #[case::query(&[q_miss("miss-key")], "/?key-exists=some-val".into())]
    #[case::query_with_method_path(&[method("PUT"), path("/path/with/query"), q_eq("key-eq", "val-eq"), q_ex("key-exists"), q_miss("miss-key")], Request::from("/path/with/query?key-eq=val-eq&key-exists=some-val").with_method("PUT"))]
    #[case::header(&[h_eq("key-eq", "val-eq")], Request::default().with_header("key-eq", "val-eq"))]
    #[case::header(&[h_ex("key-exists")], Request::default().with_header("key-exists", "some-value"))]
    #[case::header(&[h_miss("miss-key")], Request::default())]
    #[case::header_with_method_path(&[method("GET"), path("/path/with/header"), h_eq("key-eq", "val-eq"), h_ex("key-exists"), h_miss("miss-key")], Request::from("/path/with/header").with_header("key-eq", "val-eq").with_header("key-exists", "some-value"))]
    #[case::path_fragment(&[path("/path"), f_miss()], "/path".into())]
    #[case::path_fragment(&[path("/path"), f_eq("anchor")], "/path#anchor".into())]
    #[case::path_body(&[path("/without/body"), b_miss()], "/without/body".into())]
    #[case::path_body(&[b_eq("some body")], Request::default().with_body("some body"))]
    fn valid_matchers(#[case] inner: &[Matcher], #[case] request: Request) {
        let matchers = Matchers {
            inner: inner.into_iter().map(|m| m.clone()).collect(),
        };

        assert!(
            matchers.is_matched(&request),
            "Matchers {:?} should successfully match request: {}",
            matchers.inner, request
        );

        let result = matchers.validate(&request);
        assert!(
            result.is_none(),
            "Matchers {:?} should validate successfully (return None) for request: {}",
            matchers.inner, request
        )
    }

    #[rstest::rstest]
    #[case::method(&[path("/path"), method("POST")], &[method("GET")], "/path".into())]
    #[case::path(&[method("GET"), path("/wrong")], &[path("/correct")], "/correct".into())]
    #[case::query_eq(&[q_eq("key", "wrong")], &[q_eq("key", "correct")], "/?key=correct".into())]
    #[case::query_exists(&[q_ex("missing")], &[q_miss("missing")], "/?other=value".into())]
    #[case::query_miss(&[q_miss("present")], &[q_ex("present")], "/?present=value".into())]
    #[case::header_eq(&[h_eq("Content-Type", "wrong")], &[h_eq("Content-Type", "correct")], Request::default().with_header("Content-Type", "correct"))]
    #[case::header_exists(&[h_ex("missing")], &[h_miss("missing")], Request::default())]
    #[case::header_miss(&[h_miss("present")], &[h_ex("present")], Request::default().with_header("present", "value"))]
    #[case::fragment_eq(&[f_eq("wrong")], &[f_eq("correct")], "/path#correct".into())]
    #[case::fragment_miss(&[f_miss()], &[f_eq("present")], "/path#present".into())]
    #[case::body_eq(&[b_eq("wrong body")], &[b_eq("correct body")], Request::default().with_body("correct body"))]
    #[case::body_miss(&[b_miss()], &[b_eq("present")], Request::default().with_body("present"))]
    #[case::multiple(&[method("POST"), path("/wrong"), q_eq("key", "bad")], &[method("GET"), path("/correct"), q_eq("key", "good")], Request::from("/correct?key=good").with_method("GET"))]
    #[case::mixed(&[method("GET"), path("/correct"), q_eq("key", "wrong")], &[q_eq("key", "right")], Request::from("/correct?key=right").with_method("GET"))]
    #[case::mixed(&[method("POST"), path("/api"), q_ex("token")], &[method("GET"), path("/"), q_miss("token")], Request::default())]
    fn invalid_matchers(
        #[case] inner: &[Matcher],
        #[case] reports: &[Matcher],
        #[case] request: Request,
    ) {
        let matchers = Matchers {
            inner: inner.into_iter().map(|m| m.clone()).collect(),
        };
        let expected_reports: Vec<Matcher> = reports.into_iter().map(|m| m.clone()).collect();

        assert!(
            !matchers.is_matched(&request),
            "Matchers {:?} should NOT match request: {}",
            matchers.inner, request
        );

        let result = matchers.validate(&request);
        assert_eq!(
            result, 
            Some(expected_reports.clone()),
            "Matchers {:?} should report errors {:?} for request: {}\nActual result: {:?}",
            matchers.inner, expected_reports, request, result
        );
    }
}
