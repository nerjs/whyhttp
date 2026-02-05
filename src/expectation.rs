use std::collections::HashMap;

use crate::{
    matchers::{MatchReport, Matcher, Matchers},
    request::Request,
};

// Response returned for any matched expectation.
// Returned unconditionally; request validation never affects response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

// Default response used when no explicit response is configured.
// Chosen to be non-success by default to make misconfiguration visible.
impl Default for Response {
    fn default() -> Self {
        Self {
            status: 200,
            headers: Default::default(),
            body: Default::default(),
        }
    }
}

// Single routed call to this expectation.
// Stored only for later validation and reporting.
#[derive(Debug)]
struct Call {
    request: Request,
    reports: Option<Vec<MatchReport>>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ExpectationReport {
    // Expectation was configured but never called.
    NoCall,
    // Number of calls does not match configured expectation.
    MismatchTimes {
        expect: u16,
        actual: u16,
    },
    // Request matched routing but failed validating matchers.
    Matcher {
        // Request is boxed to avoid making the entire enum as large as Request.
        // Without Box, all ExpectationReport variants would have the same (large) size.
        request: Box<Request>,
        reports: Vec<MatchReport>,
    },
}

// Request expectation with fixed response and deferred validation.
// Always returns configured response; validation is performed separately.
#[derive(Debug, Default)]
pub struct Expectation {
    routing: Matchers,
    validating: Matchers,
    response: Response,
    times: Option<u16>,
    calls: Vec<Call>,
}

impl Expectation {
    pub fn set_times(&mut self, times: u16) {
        self.times = Some(times);
    }
    pub fn add_routing(&mut self, matcher: Matcher) {
        self.routing.add(matcher);
    }

    pub fn add_validating(&mut self, matcher: Matcher) {
        self.validating.add(matcher);
    }

    pub fn set_response_status(&mut self, status: u16) {
        self.response.status = status;
    }

    pub fn set_response_header<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.response.headers.insert(key.into(), value.into());
    }

    pub fn set_response_body<S: Into<String>>(&mut self, body: S) {
        self.response.body = Some(body.into());
    }

    // Checks whether request matches routing matchers.
    // Used by upper-level router to select expectation.
    pub fn matches(&self, request: &Request) -> bool {
        self.routing.matches(request)
    }

    pub fn call(&mut self, request: Request) -> Response {
        let reports = self.validating.mismatches(&request);
        self.calls.push(Call { request, reports });
        self.response.clone()
    }

    pub fn reports(&self) -> Option<Vec<ExpectationReport>> {
        if self.calls.is_empty() {
            return Some(vec![ExpectationReport::NoCall]);
        }

        let mut reports: Vec<ExpectationReport> = vec![];

        if let Some(times) = self.times
            && times as usize != self.calls.len()
        {
            reports.push(ExpectationReport::MismatchTimes {
                expect: times,
                actual: self.calls.len() as u16,
            });
        }

        for call in self.calls.iter().filter_map(|req| {
            req.reports
                .as_ref()
                .map(|reports| ExpectationReport::Matcher {
                    request: Box::new(req.request.clone()),
                    reports: reports.clone(),
                })
        }) {
            reports.push(call);
        }

        if reports.is_empty() {
            None
        } else {
            Some(reports)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::matchers::Matcher;

    #[test]
    fn reports_no_call_when_never_called() {
        let reports = Expectation::default()
            .reports()
            .expect("expected Some(NoCall)");

        assert_eq!(reports, vec![ExpectationReport::NoCall]);
    }

    #[test]
    fn call_returns_configured_response() {
        let mut e = Expectation::default();
        e.set_response_status(418);
        e.set_response_header("x-test", "1");
        e.set_response_body("ok");

        let resp = e.call(Request::default());

        assert_eq!(resp.status, 418);
        assert_eq!(resp.headers.get("x-test").map(|s| s.as_str()), Some("1"));
        assert_eq!(resp.body.as_deref(), Some("ok"));
    }

    #[test]
    fn reports_none_when_called_and_no_issues() {
        let mut e = Expectation::default();
        e.call(Request::default());

        assert!(e.reports().is_none());
    }

    #[test]
    fn reports_mismatch_times_only() {
        let mut e = Expectation::default();
        e.set_times(2);

        e.call(Request::default()); // actual = 1

        let reports = e.reports().expect("expected reports");

        assert_eq!(
            reports,
            vec![ExpectationReport::MismatchTimes {
                expect: 2,
                actual: 1
            }]
        );
    }

    #[test]
    fn reports_matcher_with_boxed_request_and_matchreports() {
        let mut e = Expectation::default();
        e.add_validating(Matcher::Method("POST".into()));

        let req = Request::default().with_method("GET");
        e.call(req.clone());

        let reports = e.reports();

        assert_eq!(
            reports,
            Some(vec![ExpectationReport::Matcher {
                request: Box::new(req),
                reports: vec![MatchReport {
                    expected: Matcher::Method("POST".to_string()),
                    actual: Matcher::Method("GET".to_string())
                }]
            }])
        );
    }

    #[test]
    fn reports_can_contain_times_and_matcher_issues() {
        let mut e = Expectation::default();
        e.set_times(2);
        e.add_validating(Matcher::Method("POST".into()));

        e.call(Request::default().with_method("GET")); // GET, and actual=1

        let reports = e.reports().expect("expected reports");
        assert_eq!(reports.len(), 2, "reports={reports:?}");

        // Current implementation pushes times mismatch first, then matcher reports.
        assert!(
            matches!(reports[0], ExpectationReport::MismatchTimes { .. }),
            "expected MismatchTimes first, got {reports:?}"
        );
        assert!(
            matches!(reports[1], ExpectationReport::Matcher { .. }),
            "expected Matcher second, got {reports:?}"
        );
    }

    #[test]
    fn routing_matches_true_when_all_routing_matchers_match() {
        let mut e = Expectation::default();
        e.add_routing(Matcher::Method("POST".into()));
        e.add_routing(Matcher::Path("/some/path".into()));

        let req = Request::default()
            .with_method("POST")
            .with_path("/some/path");
        assert!(e.matches(&req));
    }

    #[test]
    fn routing_matches_false_when_any_routing_matcher_mismatches() {
        let mut e = Expectation::default();
        e.add_routing(Matcher::Method("POST".into()));

        let req = Request::default();
        assert!(!e.matches(&req));
    }

    #[test]
    fn routing_matches_is_independent_from_validating() {
        let mut e = Expectation::default();

        e.add_routing(Matcher::Method("GET".into()));
        e.add_validating(Matcher::HeaderExists("x-missing".into()));

        let req = Request::default();

        assert!(e.matches(&req));

        e.call(req);
        let reports = e.reports().expect("expected reports");
        assert!(
            reports
                .iter()
                .any(|r| matches!(r, ExpectationReport::Matcher { .. })),
            "expected Matcher report, got {reports:?}"
        );
    }
}
