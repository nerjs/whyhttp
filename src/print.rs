macro_rules! logln {
    ($indent:expr, $level:expr, $($arg:tt)*) => {{
        let indent = $indent;
        let spaces = "  ".repeat(indent);
        println!("{}[WHYHTTP {}]: {}", spaces, $level, format!($($arg)*));
    }};
}

pub(crate) use logln;
