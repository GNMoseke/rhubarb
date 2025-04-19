pub(crate) enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}


pub(crate) fn log(msg: String, level: LogLevel) {
    match level {
        LogLevel::Debug => println!("[DEBUG]: {}", msg),
        LogLevel::Info => println!("[INFO]: {}", msg),
        LogLevel::Warning => println!("[WARNING]: {}", msg),
        LogLevel::Error => eprintln!("[ERROR]: {}", msg),
    }
}
