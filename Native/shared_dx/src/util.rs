use crate::error::{HookError, Result};
use std::time::{SystemTime};

const LOG_TIME:bool = true;

use std::cell::RefCell;
use std::collections::HashMap;

lazy_static! {
    static ref LOG_FILE_NAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    static ref LOG_INIT_TIME: std::sync::Mutex<SystemTime> = std::sync::Mutex::new(SystemTime::now());

    pub static ref LOG_EXCL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

thread_local! {
    // create a hash map that maps strings to a count of the time that string has been logged
    static LOG_ONCE: RefCell<std::collections::HashMap<String, u32>> = RefCell::new(HashMap::new());
}

pub fn set_log_file_path(path: &str, name: &str) -> Result<()> {
    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => Err(HookError::WinApiError(format!("lock error: {}", e))),
        Ok(mut fname) => {
            let mut p = path.to_owned();
            p.push_str(name);
            *fname = p;
            Ok(())
        }
    }
}

/// Return the log file path or "" if there was an error.  This function will temporarily lock
/// a global mutex protecting access to the variable.
pub fn get_log_file_path() -> String {
    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => {
            eprintln!(
                "ModelMod: derp, can't write log file due to lock error: {}",
                e
            );
            "".to_owned()
        }
        Ok(fname) => {
            (*fname).to_owned()
        }
    }
}

enum LimResult {
    Log,
    DontLog,
    DontLogAndFYI(String),
}
fn log_limit(s:&str) -> LimResult {
    const LOG_LIMIT:u32 = 25;

    LOG_ONCE.with(|log_once| {
        let mut map = log_once.borrow_mut();
        let count = map.get(s);
        match count {
            None => {
                map.insert(s.to_owned(), 1);
                LimResult::Log
            },
            Some(c) if *c > LOG_LIMIT => {
                LimResult::DontLog
            },
            Some(c) => {
                if *c == LOG_LIMIT {
                    let fyi = format!("performance warning: message '{}' has been logged {} times; it won't be repeated", s, LOG_LIMIT);
                    map.get_mut(s).map(|c| *c += 1);
                    LimResult::DontLogAndFYI(fyi)
                } else {
                    map.get_mut(s).map(|c| *c += 1);
                    LimResult::Log
                }
            }
        }
    })

}

pub fn write_log_file(msg: &str) {
    use std::env::temp_dir;
    use std::fs::OpenOptions;
    use std::io::Write;

    let alt_msg = match log_limit(msg) {
        LimResult::Log => {
            None
        },
        LimResult::DontLog => {
            return;
        },
        LimResult::DontLogAndFYI(s) => {
            Some(s)
        }
    };

    let lock = LOG_FILE_NAME.lock();
    match lock {
        Err(e) => {
            eprintln!(
                "ModelMod: derp, can't write log file due to lock error: {}",
                e
            );
        }
        Ok(mut fname) => {
            if (*fname).is_empty() {
                let mut td = temp_dir();
                println!("no log path, writing log to {:?}", td);
                td.push("ModelMod.log");
                match td.as_path().to_str() {
                    None => {
                        eprintln!("ModelMod: error getting temp path");
                        return;
                    }
                    Some(p) => {
                        *fname = p.to_owned();
                    }
                }
            }

            // set log time
            let time_ms =
                if LOG_TIME {
                    match LOG_INIT_TIME.lock() {
                        Ok(start) => {
                            let since_start =
                                SystemTime::now().duration_since(*start)
                                .unwrap_or_else(|_| std::time::Duration::from_millis(0));
                            let in_ms = since_start.as_secs() * 1000 +
                            since_start.subsec_nanos() as u64 / 1_000_000;
                            in_ms as u32
                        },
                        Err(_) => 0_u32
                    }
                } else {
                    0
                };

            let tid = std::thread::current().id();

            let w = || -> std::io::Result<()> {
                let mut f = OpenOptions::new().create(true).append(true).open(&*fname)?;
                if let Some(what) = alt_msg {
                    writeln!(f, "{:?}/{}ms: {}\r", tid, time_ms, what)?;
                } else {
                    writeln!(f, "{:?}/{}ms: {}\r", tid, time_ms, msg)?;
                }
                Ok(())
            };

            w().unwrap_or_else(|e| eprintln!("ModelMod: log file write error: {}", e));
        }
    };
}

pub trait ReleaseDrop {
    fn OnDrop(&mut self);
}

pub struct ReleaseOnDrop<T: ReleaseDrop> {
    rd: T,
}

impl<T> ReleaseOnDrop<T>
where
    T: ReleaseDrop,
{
    pub fn new(rd: T) -> Self {
        ReleaseOnDrop { rd }
    }

    pub fn as_mut(&mut self) -> &mut T {
        &mut self.rd
    }
}

impl<T> std::ops::Drop for ReleaseOnDrop<T>
where
    T: ReleaseDrop,
{
    fn drop(&mut self) {
        self.rd.OnDrop();
    }
}

#[macro_export]
macro_rules! impl_release_drop {
    ($ptrtype:ident) => {
        impl $crate::util::ReleaseDrop for *mut $ptrtype {
            fn OnDrop(&mut self) -> () {
                unsafe {
                    let ptr = *self;
                    if ptr != std::ptr::null_mut() {
                        (*ptr).Release();
                    }
                };
            }
        }
    };
}

#[cfg(test)]
// these tests require access to test internals which is nightly only
// to enable them, comment out this cfg then uncomment the 'extern crate test' line in lib.rs
mod tests {
    use super::*;

    struct Foo {
        released: bool
    }
    impl Foo {
        fn new() -> Self {
            Foo { released: false }
        }

        fn Release(&mut self) {
            self.released = true;
        }
    }

    impl_release_drop!(Foo);

    #[test]
    pub fn test_release_on_drop() {
        let mut foo = Foo::new();
        let rod = ReleaseOnDrop::new(&mut foo as *mut Foo);
        assert_eq!(foo.released, false);
        std::mem::drop(rod);
        assert_eq!(foo.released, true);

        let mut foo = Foo::new();
        {
            let _rod = ReleaseOnDrop::new(&mut foo as *mut Foo);
            assert_eq!(foo.released, false);
        }
        assert_eq!(foo.released, true);

        let mut foo = Foo::new();
        {
            let _arod;
            {
                _arod = vec![ReleaseOnDrop::new(&mut foo as *mut Foo)];
                assert_eq!(foo.released, false);
            }
        }

        assert_eq!(foo.released, true);

        {
            let _nullrod = ReleaseOnDrop::new(std::ptr::null_mut::<Foo>());
            // should not crash when above is dropped
        }

    }

    #[test]
    fn test_logging_and_limit() {
        let _loglock = LOG_EXCL_LOCK.lock().unwrap();

        let testfile = "__testutil__test_log_limit.txt";
        std::fs::remove_file(testfile).ok();

        set_log_file_path("", testfile).expect("doh");
        for _i in 0..30 {
            write_log_file("spam");
            write_log_file("spam");
            write_log_file("spam");
            write_log_file("humbug");
        }

        use std::io::Read;
        let mut f = std::fs::File::open(testfile).expect("doh");
        let mut s = String::new();
        f.read_to_string(&mut s).expect("doh");
        let lines: Vec<&str> = s.lines().collect();

        let spam = lines.iter().filter(|l| l.contains("spam")).count();
        let humbug = lines.iter().filter(|l| l.contains("humbug")).count();
        assert_eq!(spam, 26);
        assert_eq!(humbug, 26);

        let mut linei = lines.iter();
        let aftercolon = |s: &str| {
            let cidx = s.find(": ").expect("doh");
            s.split_at(cidx + ": ".len() ).1.trim().to_owned()
        };

        for i in 0..8 {
            assert_eq!(aftercolon(linei.next().unwrap()), "spam", "line {}", i);
            assert_eq!(aftercolon(linei.next().unwrap()), "spam", "line {}", i);
            assert_eq!(aftercolon(linei.next().unwrap()), "spam", "line {}", i);
            assert_eq!(aftercolon(linei.next().unwrap()), "humbug", "line {}", i);
        }
        assert_eq!(aftercolon(linei.next().unwrap()), "spam");
        let fyi = "performance warning: message 'spam' has been logged 25 times; it won't be repeated";
        assert_eq!(aftercolon(linei.next().unwrap()), fyi);
        // then this many humbug lines followed by its spam warning
        for i in 0..17 {
            assert_eq!(aftercolon(linei.next().unwrap()), "humbug", "{}", i);
        }
        let fyi = "performance warning: message 'humbug' has been logged 25 times; it won't be repeated";
        assert_eq!(aftercolon(linei.next().unwrap()), fyi);



    }
}