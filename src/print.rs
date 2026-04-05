use crate::reports::{Report, ReportReason};
macro_rules! logln {
    ($indent:expr, $level:expr, $($arg:tt)*) => {{
        let indent = $indent;
        let spaces = "  ".repeat(indent);
        println!("{}[WHYHTTP {}]: {}", spaces, $level, format!($($arg)*));
    }};
}

pub(crate) use logln;

pub fn print_report(Report { request, reasons }: Report) {
    logln!(1, "REPORT", "{request}");
    for reason in reasons {
        match reason {
            ReportReason::UnmatchedRequest => logln!(2, "DETAILS", "unexpected request"),
            ReportReason::NoCall => logln!(2, "DETAILS", "never called"),
            ReportReason::MismatchTimes { expect, actual } => {
                logln!(2, "DETAILS", "called {actual} time(s), expected {expect}");
            }
            ReportReason::Matcher { request, reports } => {
                logln!(2, "DETAILS", "validation failed on {request}");
                for report in reports {
                    logln!(
                        3,
                        "mismatch",
                        "expected {:?}, got {:?}",
                        report.expected,
                        report.actual
                    );
                }
            }
        }
    }
}
