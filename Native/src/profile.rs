#[cfg(feature = "profile")]
macro_rules! decl_profile_globals {
    ($v:ident) => {
        static mut PROFILE_BLOCKS: Option<Vec<*mut ProfileBlock>> = None;
        static mut PROFILE_ACCUM: Option<FnvHashMap<&'static str, ProfileBlockSummary>> = None;
        static mut PROFILE_SUMMARY_T: Option<SystemTime> = None;
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! decl_profile_globals { ($v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_blocks {
    ($v:ident) => {
        if PROFILE_BLOCKS.is_none() {
            PROFILE_BLOCKS = Some(Vec::with_capacity(20));
        }
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_blocks { ($v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_start {
    ($v:ident) => {
        let $v = Box::new(ProfileBlock {
            name: stringify!($v),
            start: std::time::UNIX_EPOCH,
            elapsed: std::time::Duration::from_secs(0 as u64),
        });
        let $v: *mut ProfileBlock = Box::into_raw($v);
        PROFILE_BLOCKS.as_mut().unwrap().push($v);
        (* $v).start = SystemTime::now();
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_start { ($v:ident) => {} }

#[cfg(feature = "profile")]
macro_rules! profile_end {
    ($v:ident) => {
        { (* $v).elapsed = SystemTime::now().duration_since((* $v).start).unwrap(); };
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_end {
    ($v:ident) => {
    }
}

#[cfg(feature = "profile")]
macro_rules! profile_accum {
    () => {
        if PROFILE_ACCUM.is_none() {
            PROFILE_ACCUM = Some(FnvHashMap::with_capacity_and_hasher((100) as usize, Default::default()));
        }
        for block in PROFILE_BLOCKS.as_mut().unwrap().iter_mut() {
            let block:Box<ProfileBlock> = Box::from_raw(*block);
            let secs = (*block).elapsed.as_secs() as f64 + block.elapsed.subsec_nanos() as f64 * 1e-9;
            let entry = PROFILE_ACCUM.as_mut().unwrap().entry(block.name).or_insert(ProfileBlockSummary {
                total_time: 0.0
            });

            entry.total_time += secs;

        }

        PROFILE_BLOCKS.as_mut().unwrap().clear();
    }
}
#[cfg(not(feature = "profile"))]
macro_rules! profile_accum {
    () => {
    }
}

#[cfg(feature = "profile")]
macro_rules! profile_summarize {
    ($hookdev:ident) => {
        if PROFILE_SUMMARY_T.is_none() {
            PROFILE_SUMMARY_T = Some(SystemTime::now());
        }

        // report stats every 10 seconds
        let now = SystemTime::now();
        let selapsed = now.duration_since(*PROFILE_SUMMARY_T.as_ref().unwrap()).unwrap();
        let secs = selapsed.as_secs() as f64 + selapsed.subsec_nanos() as f64 * 1e-9;
        if secs > 10.0 {
            let mut out = String::new();

            out.push_str(&format!("{} secs elapsed since last profile, curr fps: {}\r\n",
                secs, $hookdev.last_fps));
            for key in PROFILE_ACCUM.as_ref().unwrap().keys() {
                let block = PROFILE_ACCUM.as_ref().unwrap().get(key).unwrap();
                let pct = block.total_time / secs * 100.0;

                let s = format!("   {}: {} secs ({}%)\r\n", key, block.total_time, pct );
                out.push_str(&s);
            }

            write_log_file(&out);

            PROFILE_ACCUM.as_mut().unwrap().clear();
            PROFILE_SUMMARY_T = Some(now);
        }
    }
}

#[cfg(not(feature = "profile"))]
macro_rules! profile_summarize {
    ($hookdev:ident) => {
    }
}
