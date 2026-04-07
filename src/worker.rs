use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

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

pub type SharedWorker = Arc<Mutex<Worker>>;

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
                reasons: vec![ReportReason::UnmatchedRequest],
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
    use crate::matchers::test_helpers::*;

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
