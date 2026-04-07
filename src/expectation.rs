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
