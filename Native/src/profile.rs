#[cfg(feature = "profile")]
macro_rules! decl_profile_globals {
    ($v:ident) => {
        mod $v {
            use std;
            pub use fnv::FnvHashMap;

            pub struct ProfileBlock {
                pub name: &'static str,
                pub start: std::time::SystemTime,
                pub elapsed: std::time::Duration,
            }
            pub struct ProfileBlockSummary {
                pub total_time: f64
            }

            pub static mut PROFILE_BLOCKS: Option<Vec<*mut ProfileBlock>> = None;
            pub static mut PROFILE_ACCUM:
                Option<FnvHashMap<&'static str, ProfileBlockSummary>> = None;
            pub static mut PROFILE_SUMMARY_T: Option<std::time::SystemTime> = None;
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! decl_profile_globals { ($v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_blocks {
    ($modn:ident, $v:ident) => {
        if $modn::PROFILE_BLOCKS.is_none() {
            $modn::PROFILE_BLOCKS = Some(Vec::with_capacity(20));
        }
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_blocks { ($modn:ident, $v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_start {
    ($modn:ident, $v:ident) => {
        let $v = Box::new($modn::ProfileBlock {
            name: stringify!($v),
            start: std::time::UNIX_EPOCH,
            elapsed: std::time::Duration::from_secs(0 as u64),
        });
        let $v: *mut $modn::ProfileBlock = Box::into_raw($v);
        $modn::PROFILE_BLOCKS.as_mut().unwrap().push($v);
        (* $v).start = std::time::SystemTime::now();
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_start { ($modn:ident, $v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
        { (* $v).elapsed = std::time::SystemTime::now().duration_since((* $v).start).unwrap(); };
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
    }
}

#[cfg(feature = "profile")]
macro_rules! profile_accum {
    ($modn:ident) => {
        if $modn::PROFILE_ACCUM.is_none() {
            $modn::PROFILE_ACCUM = Some(
                $modn::FnvHashMap::with_capacity_and_hasher((100) as usize, Default::default()));
        }
        for block in $modn::PROFILE_BLOCKS.as_mut().unwrap().iter_mut() {
            let block:Box<$modn::ProfileBlock> = Box::from_raw(*block);
            let secs = (*block).elapsed.as_secs() as f64
                + block.elapsed.subsec_nanos() as f64 * 1e-9;
            let entry = $modn::PROFILE_ACCUM
                .as_mut().unwrap().entry(block.name).or_insert($modn::ProfileBlockSummary {
                total_time: 0.0
            });

            entry.total_time += secs;

        }

        $modn::PROFILE_BLOCKS.as_mut().unwrap().clear();
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_accum {
    ($modn:ident) => {
    }
}

#[cfg(feature = "profile")]
macro_rules! profile_summarize {
    ($modn:ident, $hookdev:ident) => {
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
            out.push_str(&format!("[Profiler {}] {} secs elapsed since last profile, curr fps: {}\r\n",
                modn, secs, $hookdev.last_fps));
            for key in $modn::PROFILE_ACCUM.as_ref().unwrap().keys() {
                let block = $modn::PROFILE_ACCUM.as_ref().unwrap().get(key).unwrap();
                let pct = block.total_time / secs * 100.0;

                let s = format!("   {}: {} secs ({}%)\r\n", key, block.total_time, pct );
                out.push_str(&s);
            }

            write_log_file(&out);

            $modn::PROFILE_ACCUM.as_mut().unwrap().clear();
            $modn::PROFILE_SUMMARY_T = Some(now);
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_summarize {
    ($modn:ident, $hookdev:ident) => {
    }
}
