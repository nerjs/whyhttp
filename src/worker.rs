use std::collections::HashMap;

use crate::{
    expectation::Expectation,
    io::{Request, Response},
    matchers::Matcher,
    reports::{Report, ReportReason},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpectationId(u16);

/// The central expectation registry and request dispatcher.
///
/// Stores expectations, routes incoming requests, returns configured responses
/// and produces aggregated reports after execution.
#[derive(Debug, Default)]
pub struct Worker {
    next_id: u16,
    expectations: HashMap<ExpectationId, Expectation>,
    unmatched_calls: Vec<Request>,
}

impl Worker {
    /// Creates and registers a new empty expectation.
    pub fn create_next(&mut self) -> ExpectationId {
        let id = ExpectationId(self.next_id);
        self.next_id += 1;
        self.expectations.insert(id.clone(), Expectation::default());
        id
    }

    pub fn set_times(&mut self, id: &ExpectationId, times: u16) {
        self.expectations.get_mut(id).unwrap().set_times(times);
    }
    pub fn add_routing(&mut self, id: &ExpectationId, matcher: Matcher) {
        self.expectations.get_mut(id).unwrap().add_routing(matcher);
    }

    pub fn add_validating(&mut self, id: &ExpectationId, matcher: Matcher) {
        self.expectations
            .get_mut(id)
            .unwrap()
            .add_validating(matcher);
    }

    pub fn set_response_status(&mut self, id: &ExpectationId, status: u16) {
        self.expectations
            .get_mut(id)
            .unwrap()
            .set_response_status(status);
    }

    pub fn set_response_header<K: Into<String>, V: Into<String>>(
        &mut self,
        id: &ExpectationId,
        key: K,
        value: V,
    ) {
        self.expectations
            .get_mut(id)
            .unwrap()
            .set_response_header(key, value);
    }

    pub fn set_response_body<S: Into<String>>(&mut self, id: &ExpectationId, body: S) {
        self.expectations
            .get_mut(id)
            .unwrap()
            .set_response_body(body);
    }

    pub fn remove(&mut self, id: &ExpectationId) {
        self.expectations.remove(id);
    }

    /// Routes a request to the first matching expectation.
    ///
    /// Returns the matched response, or the default response when
    /// no expectation matches the request.
    pub fn handle(&mut self, request: Request) -> Response {
        for id in 0..self.next_id {
            let id = ExpectationId(id);
            let Some(expectation) = self.expectations.get_mut(&id) else {
                continue;
            };

            if expectation.matches(&request) {
                return expectation.call(request);
            }
        }
        self.unmatched_calls.push(request);
        Response::default()
    }

    /// Returns a report for a specific expectation, if present.
    pub fn single_report(&self, id: &ExpectationId) -> Option<Report> {
        self.expectations.get(id)?.reports()
    }

    /// Returns all collected reports, including unmatched incoming requests.
    pub fn reports(&self) -> Option<Vec<Report>> {
        let mut reports: Vec<Report> = Vec::new();

        for id in 0..self.next_id {
            let Some(report) = self.single_report(&ExpectationId(id)) else {
                continue;
            };
            reports.push(report);
        }

        for request in self.unmatched_calls.iter() {
            reports.push(Report {
                request: request.clone(),
                reasons: vec![ReportReason::NotSetup],
            });
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
    use crate::matchers::{MatchReport, test_helpers::*};
    use rstest::rstest;

    #[test]
    fn increment_expectation_ids() {
        let mut worker = Worker::default();

        let first = worker.create_next();
        let second = worker.create_next();

        assert_ne!(first, second);
        assert_eq!(first.0, 0);
        assert_eq!(second.0, 1);
    }

    #[test]
    fn handle_with_default_response_without_expectations() {
        let mut worker = Worker::default();

        let response = worker.handle(Request::default());
        assert_eq!(response, Response::default());
    }

    #[test]
    fn returns_configured_status_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.set_response_status(&id, 222);

        let response = worker.handle(Request::default());
        assert_eq!(response.status, 222);
    }

    #[test]
    fn returns_configured_header_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.set_response_header(&id, "some-key", "some-value");

        let response = worker.handle(Request::default());
        let header_value = response.headers.get("some-key").map(String::as_str);
        assert_eq!(header_value, Some("some-value"));
    }

    #[test]
    fn returns_configured_body_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.set_response_body(&id, "some body text");

        let response = worker.handle(Request::default());
        assert_eq!(response.body, Some("some body text".to_string()));
    }

    #[test]
    fn returns_full_configured_response_from_matched_expectation() {
        let mut worker = Worker::default();
        let id = worker.create_next();
        worker.set_response_status(&id, 222);
        worker.set_response_header(&id, "some-key", "some-value");
        worker.set_response_body(&id, "some body text");

        let response = worker.handle(Request::default());

        assert_eq!(response.status, 222);

        let header_value = response.headers.get("some-key").map(String::as_str);
        assert_eq!(header_value, Some("some-value"));

        assert_eq!(response.body, Some("some body text".to_string()));
    }

    #[test]
    fn returns_default_response_when_no_expectation_matches_request() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/first".to_string()));
        worker.set_response_status(&id, 301);

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/second".to_string()));
        worker.set_response_status(&id, 302);

        let response = worker.handle(Request::default().with_path("/other"));

        assert_eq!(response.status, Response::default().status);
    }

    #[rstest]
    #[case::method(method("PUT"), vec![method("GET"), method("post"), method("DELETE")], Request::default().with_method("put"))]
    #[case::path(path("/need/to/call"), vec![path("/first"), path("/second")], Request::default().with_path("/need/to/call"))]
    #[case::query_eq(q_eq("key", "val"), vec![q_miss("key"), q_ex("other_key"), q_eq("key", "other val")], Request::default().with_query("key", Some("val")))]
    #[case::query_ex(q_ex("key"), vec![q_miss("key"), q_ex("other_key"), q_eq("other-key", "other val")], Request::default().with_query("key", Some("val")))]
    #[case::query_miss(q_miss("key"), vec![q_ex("other_key"), q_eq("other-key", "other val"), q_eq("key", "val")], Request::default().with_query("key2", Some("val2")))]
    #[case::header_eq(h_eq("key", "val"), vec![h_ex("key2"), h_miss("key"), h_eq("key", "val2")], Request::default().with_header("key", "val"))]
    #[case::header_ex(h_ex("key"), vec![h_ex("key2"), h_miss("key"), h_eq("key2", "val2")], Request::default().with_header("key", "val"))]
    #[case::header_miss(h_miss("key"), vec![h_ex("key"), h_miss("key2"), h_eq("key", "val")], Request::default().with_header("key2", "val"))]
    #[case::body_eq(b_eq("some body"), vec![b_miss(), b_eq("some other body")], Request::default().with_body("some body"))]
    #[case::body_miss(b_miss(), vec![b_eq("some body")], Request::default())]
    fn routes_request_to_matching_expectation(
        #[case] correct_matcher: Matcher,
        #[case] incorrect_matchers: Vec<Matcher>,
        #[case] request: Request,
    ) {
        let mut worker = Worker::default();

        let mut status_num = 600;
        for m in incorrect_matchers.into_iter() {
            status_num += 1;
            let id = worker.create_next();
            worker.add_routing(&id, m);
            worker.set_response_status(&id, status_num);
        }

        let correct_status = status_num + 1;
        let id = worker.create_next();
        worker.add_routing(&id, correct_matcher);
        worker.set_response_status(&id, correct_status);

        let response = worker.handle(request);

        assert_eq!(response.status, correct_status);
    }

    #[test]
    #[should_panic]
    fn panics_when_setting_times_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.set_times(&ExpectationId(222), 2);
    }

    #[test]
    #[should_panic]
    fn panics_when_adding_routing_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.add_routing(&ExpectationId(222), path("/"));
    }
    #[test]
    #[should_panic]
    fn panics_when_adding_validating_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.add_validating(&ExpectationId(222), method("get"));
    }

    #[test]
    #[should_panic]
    fn panics_when_setting_response_status_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.set_response_status(&ExpectationId(222), 222);
    }
    #[test]
    #[should_panic]
    fn panics_when_setting_response_header_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.set_response_header(&ExpectationId(222), "key", "value");
    }
    #[test]
    #[should_panic]
    fn panics_when_setting_response_body_for_unknown_expectation() {
        let mut worker = Worker::default();
        worker.set_response_body(&ExpectationId(222), "");
    }

    #[test]
    fn first_matching_expectation_wins() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, path("/path"));
        worker.set_response_status(&id, 222);

        let id = worker.create_next();
        worker.add_routing(&id, path("/path"));
        worker.set_response_status(&id, 333);

        let response = worker.handle(Request::default().with_path("/path"));

        assert_eq!(response.status, 222);
    }

    #[test]
    fn routes_request_to_matching_expectation_by_method_and_path() {
        let mut worker = Worker::default();

        // This is the specific case that should be handled if both conditions are met
        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/as/needed".to_string()));
        worker.add_routing(&id, Matcher::Method("put".to_string()));
        worker.set_response_status(&id, 301);

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/as/needed".to_string()));
        worker.set_response_status(&id, 302);

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Method("put".to_string()));
        worker.set_response_status(&id, 303);

        let response = worker.handle(Request::default().with_path("/as/needed"));
        assert_eq!(response.status, 302);

        let response = worker.handle(Request::default().with_method("PUT"));
        assert_eq!(response.status, 303);

        let response = worker.handle(
            Request::default()
                .with_path("/as/needed")
                .with_method("PUT"),
        );
        assert_eq!(response.status, 301);
    }

    #[test]
    fn skips_removed_expectation_during_routing() {
        let mut worker = Worker::default();

        let skip_id = worker.create_next();
        worker.add_routing(&skip_id, Matcher::Path("/path".to_string()));
        worker.set_response_status(&skip_id, 222);

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/path".to_string()));
        worker.set_response_status(&id, 223);

        worker.remove(&skip_id);

        let response = worker.handle(Request::default().with_path("/path"));

        assert_eq!(response.status, 223);
    }

    #[test]
    fn removing_nonexistent_expectation_does_not_affect_routing() {
        let mut worker = Worker::default();

        // Should not cause errors
        worker.remove(&ExpectationId(222));
    }

    #[test]
    fn removing_same_expectation_twice_is_harmless() {
        let mut worker = Worker::default();

        let id = ExpectationId(222);
        worker.remove(&id);
        worker.remove(&id);
    }

    #[test]
    #[should_panic]
    fn removing_existing_expectation_prevents_future_configuration() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.set_response_status(&id, 222);

        worker.remove(&id);
        worker.set_response_body(&id, "");
    }

    #[test]
    fn allows_setting_times_without_affecting_response_handling() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.set_response_status(&id, 301);
        worker.set_times(&id, 12);

        let response = worker.handle(Request::default());
        assert_eq!(response.status, 301);
    }

    #[test]
    fn validating_matchers_do_not_prevent_response_from_matched_expectation() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_validating(&id, Matcher::Method("put".to_string()));
        worker.set_response_status(&id, 301);

        let response = worker.handle(Request::default().with_method("post"));
        assert_eq!(response.status, 301);
    }

    #[test]
    fn reports_none_when_no_expectations_and_no_calls() {
        let worker = Worker::default();
        let reports = worker.reports();

        assert_eq!(reports, None);
    }

    #[test]
    fn reports_no_setuped_for_unmatched_request() {
        let mut worker = Worker::default();

        let request1 = Request::default().with_path("/path/one");
        let request2 = Request::default().with_path("/path/two");

        let _ = worker.handle(request1.clone());
        let _ = worker.handle(request2.clone());
        let reports = worker.reports();

        assert_eq!(
            reports,
            Some(vec![
                Report {
                    request: request1,
                    reasons: vec![ReportReason::NotSetup]
                },
                Report {
                    request: request2,
                    reasons: vec![ReportReason::NotSetup]
                }
            ])
        )
    }

    #[test]
    fn reports_no_call_for_never_called_expectation() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));

        let reports = worker.reports();
        assert_eq!(
            reports,
            Some(vec![Report {
                request: Request::default().with_path("/some"),
                reasons: vec![ReportReason::NoCall]
            }])
        );
    }

    #[test]
    fn reports_none_for_successful_expectation() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.add_validating(&id, Matcher::Method("PUT".to_string()));

        let _ = worker.handle(Request::default().with_path("/some").with_method("PUT"));

        let reports = worker.reports();

        assert_eq!(reports, None);
    }

    #[test]
    fn reports_mismatch_times() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.set_times(&id, 2);

        let request = Request::default().with_path("/some");
        let _ = worker.handle(request.clone());

        let reports = worker.reports();

        assert_eq!(
            reports,
            Some(vec![Report {
                request,
                reasons: vec![ReportReason::MismatchTimes {
                    expect: 2,
                    actual: 1
                }]
            }])
        );
    }

    #[test]
    fn reports_none_when_times_match_expected_calls() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.set_times(&id, 2);

        let _ = worker.handle(Request::default().with_path("/some"));
        let _ = worker.handle(Request::default().with_path("/some"));

        let reports = worker.reports();

        assert_eq!(reports, None);
    }

    #[test]
    fn reports_matcher_reason_when_validation_fails() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.add_validating(&id, Matcher::Method("PUT".to_string()));

        let request = Request::default().with_path("/some");
        let _ = worker.handle(request.clone().with_method("POST"));

        let reports = worker.reports();

        assert_eq!(
            reports,
            Some(vec![Report {
                request: request.clone(),
                reasons: vec![ReportReason::Matcher {
                    request: Box::new(request.with_method("POST")),
                    reports: vec![MatchReport {
                        expected: Matcher::Method("PUT".to_string()),
                        actual: Matcher::Method("POST".to_string())
                    }]
                }]
            }])
        );
    }

    #[test]
    fn reports_combines_expectation_issues_and_no_setuped_calls() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.add_validating(&id, Matcher::Method("PUT".to_string()));

        let request1 = Request::default().with_path("/some");
        let _ = worker.handle(request1.clone().with_method("POST"));
        let request2 = Request::default().with_path("/some/other");
        let _ = worker.handle(request2.clone());

        let reports = worker.reports();

        assert_eq!(
            reports,
            Some(vec![
                Report {
                    request: request1.clone(),
                    reasons: vec![ReportReason::Matcher {
                        request: Box::new(request1.with_method("POST")),
                        reports: vec![MatchReport {
                            expected: Matcher::Method("PUT".to_string()),
                            actual: Matcher::Method("POST".to_string())
                        }]
                    }]
                },
                Report {
                    request: request2,
                    reasons: vec![ReportReason::NotSetup]
                }
            ])
        );
    }

    #[test]
    fn reports_skip_removed_expectations() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));
        worker.remove(&id);

        let reports = worker.reports();
        assert_eq!(reports, None);
    }

    #[test]
    fn single_report_returns_report_for_specific_expectation() {
        let mut worker = Worker::default();

        let id = worker.create_next();
        worker.add_routing(&id, Matcher::Path("/some".to_string()));

        let second_id = worker.create_next();
        worker.add_routing(&second_id, Matcher::Path("/some/other".to_string()));

        let report = worker.single_report(&second_id);
        assert_eq!(
            report,
            Some(Report {
                request: Request::default().with_path("/some/other"),
                reasons: vec![ReportReason::NoCall]
            })
        )
    }
}
