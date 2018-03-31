#[cfg(feature = "profile")]
macro_rules! decl_profile_globals {
    ($v:ident) => {
        mod $v {
            use std;
            pub use fnv::FnvHashMap;

            pub struct ProfileBlock {
                pub name: &'static str,
                pub start: std::time::SystemTime,
                pub total_time: f64
            }

            pub static mut PROFILE_ACCUM:
                Option<FnvHashMap<&'static str, ProfileBlock>> = None;
            pub static mut PROFILE_SUMMARY_T: Option<std::time::SystemTime> = None;
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! decl_profile_globals { ($v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_start {
    ($modn:ident, $v:ident) => {

        let $v = unsafe {
            let name = stringify!($v);
            if $modn::PROFILE_ACCUM.is_none() {
                $modn::PROFILE_ACCUM = Some(
                    $modn::FnvHashMap::with_capacity_and_hasher((100) as usize, Default::default()));
            }

            let accum = $modn::PROFILE_ACCUM
                        .as_mut().unwrap();
            let $v = accum.entry(name).or_insert({
                            $modn::ProfileBlock {
                                name: name,
                                start: std::time::UNIX_EPOCH,
                                total_time: 0.0,
                            }
                        });

            $v.start = std::time::SystemTime::now();
            $v
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_start { ($modn:ident, $v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
        let elapsed = std::time::SystemTime::now().duration_since($v.start).unwrap();

        let secs = elapsed.as_secs() as f64
                    elapsed.subsec_nanos() as f64 * 1e-9;
        $v.total_time += secs;
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
    }
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
            let selapsed = now.duration_since(*$modn::PROFILE_SUMMARY_T.as_ref().unwrap()).unwrap();
            let secs = selapsed.as_secs() as f64 + selapsed.subsec_nanos() as f64 * 1e-9;
            if secs > 10.0 {
                let mut out = String::new();

                let modn = stringify!($modn);
                out.push_str(&format!("[Profiler {}] {} secs elapsed since last profile\r\n",
                    modn, secs));
                for (name,block) in $modn::PROFILE_ACCUM.as_mut().unwrap().iter_mut() {
                    let pct = block.total_time / secs * 100.0;

                    let s = format!("   {}: {} secs ({}%)\r\n", name, block.total_time, pct );
                    out.push_str(&s);

                    block.total_time = 0.0; // reset for next round
                }

                write_log_file(&out);

                $modn::PROFILE_SUMMARY_T = Some(now);
            }
        };
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_summarize {
    ($modn:ident) => {
    }
}
