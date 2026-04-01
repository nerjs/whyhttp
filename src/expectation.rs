use crate::{
    io::{Request, Response},
    matchers::{MatchReport, Matcher, Matchers},
    reports::{Report, ReportReason},
};

// Single routed call to this expectation.
// Stored only for later validation and reporting.
#[derive(Debug)]
struct Call {
    request: Request,
    reports: Option<Vec<MatchReport>>,
}

/// A single request expectation with routing, validation and a fixed response.
///
/// Routing matchers decide whether the expectation is selected.
/// Validating matchers do not affect response handling and are used only
/// for mismatch reporting after the call is recorded.
#[derive(Debug, Default)]
pub struct Expectation {
    routing: Matchers,
    validating: Matchers,
    response: Response,
    times: Option<u16>,
    calls: Vec<Call>,
}

impl Expectation {
    /// Sets the expected number of calls for this expectation.
    pub fn set_times(&mut self, times: u16) {
        self.times = Some(times);
    }
    /// Adds a matcher used for request routing.
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

    /// Returns `true` when the request matches all routing matchers.
    pub fn matches(&self, request: &Request) -> bool {
        self.routing.matches(request)
    }

    /// Records a matched call and returns the configured response.
    ///
    /// Validation mismatches are stored for later reporting.
    pub fn call(&mut self, request: Request) -> Response {
        let reports = self.validating.mismatches(&request);
        self.calls.push(Call { request, reports });
        self.response.clone()
    }

    /// Builds a report for this expectation, if any issues were detected.
    ///
    /// Returns `None` when the expectation has no reportable problems.
    pub fn reports(&self) -> Option<Report> {
        let request = self.routing.to_request();
        if self.calls.is_empty() {
            return Some(Report {
                request,
                reasons: vec![ReportReason::NoCall],
            });
        }

        let mut reasons: Vec<ReportReason> = vec![];

        if let Some(times) = self.times
            && times as usize != self.calls.len()
        {
            reasons.push(ReportReason::MismatchTimes {
                expect: times,
                actual: self.calls.len() as u16,
            });
        }

        for call in self.calls.iter().filter_map(|req| {
            req.reports.as_ref().map(|reports| ReportReason::Matcher {
                request: Box::new(req.request.clone()),
                reports: reports.clone(),
            })
        }) {
            reasons.push(call);
        }

        if reasons.is_empty() {
            None
        } else {
            Some(Report { request, reasons })
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

        assert_eq!(
            reports,
            Report {
                request: Request::default().with_path("*"),
                reasons: vec![ReportReason::NoCall],
            }
        );
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
            Report {
                request: Request::default().with_path("*"),
                reasons: vec![ReportReason::MismatchTimes {
                    expect: 2,
                    actual: 1,
                }],
            }
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
            Some(Report {
                request: Request::default().with_path("*"),
                reasons: vec![ReportReason::Matcher {
                    request: Box::new(req),
                    reports: vec![MatchReport {
                        expected: Matcher::Method("POST".to_string()),
                        actual: Matcher::Method("GET".to_string()),
                    }],
                }],
            })
        );
    }

    #[test]
    fn reports_can_contain_times_and_matcher_issues() {
        let mut e = Expectation::default();
        e.set_times(2);
        e.add_validating(Matcher::Method("POST".into()));

        e.call(Request::default().with_method("GET")); // GET, and actual=1

        let reports = e.reports().expect("expected reports");
        assert_eq!(reports.reasons.len(), 2, "reports={reports:?}");

        // Current implementation pushes times mismatch first, then matcher reports.
        assert!(
            matches!(reports.reasons[0], ReportReason::MismatchTimes { .. }),
            "expected MismatchTimes first, got {reports:?}"
        );
        assert!(
            matches!(reports.reasons[1], ReportReason::Matcher { .. }),
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
                .reasons
                .iter()
                .any(|r| matches!(r, ReportReason::Matcher { .. })),
            "expected Matcher report, got {reports:?}"
        );
    }
}
