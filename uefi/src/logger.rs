#[allow(dead_code)]
pub enum Level {
    Dbug,
    Info,
    Warn,
    Erro,
    Fatl,
}

impl Level {
    fn tag(&self) -> &'static str {
        match self {
            Level::Dbug => "DBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Erro => "ERRO",
            Level::Fatl => "FATL",
        }
    }
}

fn log(level: Level, args: core::fmt::Arguments) {
    uefi::println!("[{}] {}", level.tag(), args);
}

#[macro_export]
macro_rules! dbug {
    ($($arg:tt)*) => {
        $crate::logger::_log($crate::logger::Level::Dbug, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! lg_info {
    ($($arg:tt)*) => {
        $crate::logger::_log($crate::logger::Level::Info, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::logger::_log($crate::logger::Level::Warn, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! erro {
    ($($arg:tt)*) => {
        $crate::logger::_log($crate::logger::Level::Erro, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! fatl {
    ($($arg:tt)*) => {
        $crate::logger::_log($crate::logger::Level::Fatl, format_args!($($arg)*))
    };
}

#[doc(hidden)]
pub fn _log(level: Level, args: core::fmt::Arguments) {
    log(level, args);
}
