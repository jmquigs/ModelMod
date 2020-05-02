#[cfg(feature = "profile")]
macro_rules! decl_profile_globals {
    ($v:ident) => {
        mod $v {
            pub use fnv::FnvHashMap;
            use std;

            pub struct ProfileBlock {
                pub name: &'static str,
                pub start: std::time::SystemTime,
                pub total_time: f64,
                pub count: u64,
            }

            pub static mut PROFILE_ACCUM: Option<FnvHashMap<&'static str, ProfileBlock>> = None;
            pub static mut PROFILE_SUMMARY_T: Option<std::time::SystemTime> = None;
        }
    };
}

#[cfg(not(feature = "profile"))]
macro_rules! decl_profile_globals {
    ($v:ident) => {};
}

#[cfg(feature = "profile")]
macro_rules! profile_start {
    ($modn:ident, $v:ident) => {

        let $v = unsafe {
            let name = stringify!($v);
            if $modn::PROFILE_ACCUM.is_none() {
                $modn::PROFILE_ACCUM = Some(
                    $modn::FnvHashMap::with_capacity_and_hasher((100) as usize,
                    Default::default()));
            }

            let accum = $modn::PROFILE_ACCUM
                        .as_mut().unwrap();
            let $v = accum.entry(name).or_insert({
                            $modn::ProfileBlock {
                                name: name,
                                start: std::time::UNIX_EPOCH,
                                total_time: 0.0,
                                count: 0,
                            }
                        });

            $v.start = std::time::SystemTime::now();
            $v.count += 1;
            $v
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_start {
    ($modn:ident, $v:ident) => {};
}

#[cfg(feature = "profile")]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
        let now = std::time::SystemTime::now();
        let elapsed = now.duration_since($v.start).unwrap();

        let secs = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;

        $v.total_time += secs;
    };
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {};
}

#[cfg(feature = "profile")]
macro_rules! profile_summarize {
    ($modn:ident) => {
        unsafe {
            if $modn::PROFILE_SUMMARY_T.is_none() {
                $modn::PROFILE_SUMMARY_T = Some(SystemTime::now());
            }

            // report stats every 10 seconds
            let now = SystemTime::now();
            let selapsed = now.duration_since(*$modn::PROFILE_SUMMARY_T.as_ref().unwrap())
                .unwrap();
            let secs = selapsed.as_secs() as f64 + selapsed.subsec_nanos() as f64 * 1e-9;
            if secs > 10.0 {
                let mut out = String::new();

                let modn = stringify!($modn);
                out.push_str(&format!(
                    "[Profiler {}] {} secs elapsed since last profile\r\n",
                    modn, secs
                ));
                for (name, block) in $modn::PROFILE_ACCUM.as_mut().unwrap().iter_mut() {
                    let pct = block.total_time / secs * 100.0;

                    let s = format!(
                        "   {}: {} secs ({}%) (count: {})\r\n",
                        name, block.total_time, pct, block.count
                    );
                    out.push_str(&s);

                    block.total_time = 0.0; // reset for next round
                }

                write_log_file(&out);

                $modn::PROFILE_SUMMARY_T = Some(now);
            }
        };
    };
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_summarize {
    ($modn:ident) => {};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std;
    use std::time::SystemTime;
    use util::*;

    decl_profile_globals!(test_profiler);

    #[test]
    fn profile_works() {
        set_log_file_path("", "testlog.txt");
        const sleeptime: u32 = 250;
        let secs = 1000 / sleeptime;
        let itersec = 16;
        for _i in 0..(itersec * secs) {
            profile_start!(test_profiler, main);
            std::thread::sleep_ms(sleeptime);
            profile_end!(test_profiler, main);

            profile_summarize!(test_profiler);
        }
    }
}
