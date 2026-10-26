//! Coloured console logging with timestamps.

use std::fmt::Display;

use chrono::Utc;

#[allow(clippy::upper_case_acronyms)]
pub enum LVL {
    Info,
    Debug,
    Warn,
    Error,
    Success,
    Other { mtype: Mtype },
}

pub enum Mtype {
    Header,
    Content,
}

pub fn log<D: Display>(level: LVL, message: D) {
    use LVL::*;

    let color = match level {
        Info => "1;38;5;136m",
        Debug => "1;38;5;4m",
        Warn => "1;38;5;178m",
        Error => "1;38;5;196m",
        Success => "1;38;5;34m",
        Other { mtype } => {
            match mtype {
                Mtype::Header => println!("--- {message} ---"),
                Mtype::Content => println!("    {message}"),
            }
            return;
        }
    };

    let log_time = Utc::now().time().format("%H:%M:%S");
    println!("\x1b[{color}[{log_time}]:\x1b[0m {message}");
}
