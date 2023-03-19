/*!
A simple sampling profile.

Reports time spent in various blocks marked by `profile_start!` and `profile_end!`.
Reports are written to the log file every 10 seconds (or as specified in the
`profile_summarize!` macro).  Time reported is inclusive only (that is, each block tracks
the time spent in all subblocks referenced while it is active).

Normally the profiler is disabled and its various macros compile to nothing, which eliminates
any performance cost from them.

To enable it, add "profile" to the default features for this crate in Cargo.toml, as well
as any crates you want to profile (usually hook_core).

Then run the game and let it sit in a visually complex scene for a couple minutes.

An extremely crude way to examine the results is to pick some block name and search the log
for it using git bash tools, like so:

cat logfilename | grep -i "mod_precheck" | grep -v post | awk '{print $4}' | sort | uniq -c

Which prints results like

      3 (0.0%)
      1 (0.2%)
      1 (0.3%)
      5 (0.4%)
     12 (0.5%)

showing that in 12 of the samples, this particular block took 0.5% of the total time, which given
the 10 second reporting interval is about 50ms in each of those samples.

Using the profiler has a performance cost, and adding or removing blocks changes this cost.
When optimizing it is best to not add or remove too many blocks,
so that the cost of the profiler itself is relatively fixed.

Every `profile_start!` should have (at least one) `profile_end!`, more than one may be necessary
if there are multiple return paths that could end the block.  If the profile gives you screwy
results, you are probably missing a `profile_end!`.

The profiler is not thread safe, since it uses global statics to track state.

*/

#[cfg(feature = "profile")]
#[macro_export]
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
#[macro_export]
macro_rules! decl_profile_globals {
    ($v:ident) => {};
}

#[cfg(feature = "profile")]
#[macro_export]
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
        };
    }
}

#[cfg(not(feature = "profile"))]
#[macro_export]
macro_rules! profile_start {
    ($modn:ident, $v:ident) => {};
}

#[cfg(feature = "profile")]
#[macro_export]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {
        let now = std::time::SystemTime::now();
        let elapsed = now.duration_since($v.start).unwrap();

        let secs = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;

        $v.total_time += secs;
    };
}
#[cfg(not(feature = "profile"))]
#[macro_export]
macro_rules! profile_end {
    ($modn:ident, $v:ident) => {};
}

#[cfg(feature = "profile")]
#[macro_export]
macro_rules! profile_summarize {
    ($modn:ident, $minsec:expr) => {
        unsafe {
            use std::time::SystemTime;
            if $modn::PROFILE_SUMMARY_T.is_none() {
                $modn::PROFILE_SUMMARY_T = Some(SystemTime::now());
            }

            // report stats every 10 seconds
            let now = SystemTime::now();
            let selapsed = now.duration_since(*$modn::PROFILE_SUMMARY_T.as_ref().unwrap())
                .unwrap();
            let secs = selapsed.as_secs() as f64 + selapsed.subsec_nanos() as f64 * 1e-9;
            if secs > $minsec {
                let mut out = String::new();

                let modn = stringify!($modn);
                out.push_str(&format!(
                    "[Profiler {}] {:3.1} secs elapsed since last profile\r\n",
                    modn, secs
                ));
                let mut blocks:Vec<&mut $modn::ProfileBlock> =
                    $modn::PROFILE_ACCUM.as_mut().map(|hm| hm.iter_mut().map(|(_, v)| v).collect()).unwrap_or_else(|| vec![]);
                blocks.sort_by(|b, a| a.total_time.partial_cmp(&b.total_time).unwrap());
                for block in blocks {
                    let pct = block.total_time / secs * 100.0;


                    let s = format!(
                        "   {}: {:3.2} secs ({:3.1}%) (count: {})\r\n",
                        block.name, block.total_time, pct, block.count
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
#[macro_export]
macro_rules! profile_summarize {
    ($modn:ident, $minsec:expr) => {};
}

#[cfg(test)]
mod tests {
    decl_profile_globals!(test_profiler);

    #[test]
    #[cfg(feature = "profile")]
    #[cfg_attr(feature = "ci", ignore)]
    fn profile_works() {
        use shared_dx::util::*;

        let _loglock = LOG_EXCL_LOCK.lock().unwrap();

        // remove previous summary file
        let testfile = "__testprofiler__test_prof_summary_log.txt";
        std::fs::remove_file(testfile).ok();

        set_log_file_path("", testfile).expect("doh");
        const SLEEPTIME:u64 = 250;
        let secs = 1000 / SLEEPTIME;
        let itersec = 8;
        profile_summarize!(test_profiler,1.0); // sets the time since last summary but doesn't print

        println!("{} iters for {} each, total = {}", (itersec * secs), SLEEPTIME, (itersec * secs * SLEEPTIME));
        // could just iterate once per second but want to simulate entering each block more
        // frequently
        for _i in 0..(itersec * secs) {
            // half the time we are outside of a block
            std::thread::sleep(std::time::Duration::from_millis(SLEEPTIME/2));

            // then spend half in main1 and 2, with 2 subdivided into 2 parts
            profile_start!(test_profiler, top);
            profile_start!(test_profiler, main1);
            std::thread::sleep(std::time::Duration::from_millis(SLEEPTIME/4));
            profile_end!(test_profiler, main1);
            profile_start!(test_profiler, main2);
            profile_start!(test_profiler, main2_sub1);
            std::thread::sleep(std::time::Duration::from_millis(SLEEPTIME/8));
            profile_end!(test_profiler, main2_sub1);
            profile_start!(test_profiler, main2_sub2);
            std::thread::sleep(std::time::Duration::from_millis(SLEEPTIME/8));
            profile_end!(test_profiler, main2_sub2);
            profile_end!(test_profiler, main2);
            profile_end!(test_profiler, top);
        }
        profile_summarize!(test_profiler,1.0); // time doesn't matter here just want file to dump

        // read all lines from testlog.txt
        use std::{io::Read, slice::Iter};
        let mut f = std::fs::File::open(testfile).expect("doh");
        let mut contents = String::new();
        f.read_to_string(&mut contents).expect("doh");
        // write the results to stdout in case of fail
        println!("{}", contents);
        let lines:Vec<&str> = contents.split("\r\n").collect();
        let mut lines = lines.iter();
        let split_secs = |line:&str| {
            let mut split = line.split("secs");
            let first = split.next().unwrap().trim();
            let mut split = first.rsplit(" ");
            let count = split.next().unwrap().parse::<f64>().unwrap();
            count
        };
        let next_secs = |lines:&mut Iter<&str>| {
            lines.next().map(|line| split_secs(line)).unwrap()
        };
        let near = |a:f64, b:f64, ep:f64| {
            let diff = (a-b).abs();
            diff < ep
        };

        // this is obviously fuzzy but if the numbers are way off from this maybe its a bug
        let n = next_secs(&mut lines); assert!(near(9.0, n, 1.0), "line 1: {}", n); // total time (first line)
        let n = next_secs(&mut lines); assert!(near(5.0, n, 0.75), "line 2: {}", n); // top time == total
        let n = next_secs(&mut lines); assert!(near(2.5, n, 0.5), "line 3: {}", n); // main 1 or 2
        let n = next_secs(&mut lines); assert!(near(2.5, n, 0.5), "line 4: {}", n); // main 1 or 2
        let n = next_secs(&mut lines); assert!(near(1.25, n, 0.25), "line 5: {}", n); // main2 sub
        let n = next_secs(&mut lines); assert!(near(1.25, n, 0.25), "line: 6: {}", n); // main2 sub

    }
}
