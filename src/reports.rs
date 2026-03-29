use crate::{matchers::MatchReport, request::Request};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Report {
    pub request: Request,
    pub reasons: Vec<ReportReason>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ReportReason {
    // запрос по неизвестному пути
    NoSetuped,

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
