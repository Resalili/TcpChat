use std::io::Write;
use std::sync::Mutex;
use std::fs::OpenOptions;

pub static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init() {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("tspchat.log")
        .expect("не вдалось відкрити лог-файл");
    *LOG_FILE.lock().unwrap() = Some(file);
}

// внутрішня функція, яку викликають макроси — не для прямого використання
pub fn write_line(level: &str, msg: &std::fmt::Arguments) {
    if let Some(file) = LOG_FILE.lock().unwrap().as_mut() {
        let _ = writeln!(file, "[{level}] {msg}");
    }
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logger::write_line("INFO", &format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::logger::write_line("ERROR", &format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::logger::write_line("DEBUG", &format_args!($($arg)*))
    };
}
