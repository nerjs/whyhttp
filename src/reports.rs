use crate::{io::Request, matchers::MatchReport};

/// A report describing problems related to a single expected or actual request.
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Report {
    pub request: Request,
    pub reasons: Vec<ReportReason>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ReportReason {
    // Request arrived but no expectation matched it.
    UnmatchedRequest,

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
        // Without Box, all ReportReason variants would have the same (large) size.
        request: Box<Request>,
        reports: Vec<MatchReport>,
    },
}
