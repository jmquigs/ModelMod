//! This module allows stats to be accumulated on which mods are being used and how long.
//! It creates a file called `modstats.$ExeBaseName.log` in the modelmod logs folder.
//! The format is the time an active mod was first noticed, the name, and the total time it
//! has been active.
//!
//! A separate thread is used to update the file so the main thread doesn't block or slow down.
//! The main thread just keeps track of mods and times and sends updates to the log thread.
//!
//! The log thread uses a somewhat complicated scheme to update the file (code in `process_mod_msgs`),
//! since we are only interested in updating the last few lines and I wanted to avoid reading the
//! whole file and writing it out again every time.
//!
//! A mod that was active but stops rendering will create a new entry in the log if it doesn't
//! render for 4 minutes but then comes back.
use std::cell::{RefCell};
use std::io::{Seek, Read, SeekFrom};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::JoinHandle;
use std::time::{SystemTime, Duration};
use std::collections::{HashMap, HashSet};

use global_state::GLOBAL_STATE;
use shared_dx::util::write_log_file;

use crate::hook_device::mm_verify_load;

struct LogThread {
    pub sender: Sender<ThreadCommand>,
    pub receiver: Receiver<ThreadReply>,
    pub thread: JoinHandle<()>,
}

struct ModStats {
    pub last_render_update: SystemTime,
    pub last_rendered: HashMap<String, (SystemTime, SystemTime, Duration)>, // start, last update, total time
    pub log_thread: Option<LogThread>,
}
impl ModStats {
    pub fn new() -> ModStats {
        ModStats {
            last_render_update: SystemTime::now(),
            last_rendered: HashMap::new(),
            log_thread: None,
        }
    }
}
enum ModMsg {
    NewModActive(String, SystemTime),
    ModActive(String, SystemTime, Duration),
}
enum ThreadCommand {
    Stop,
    ModMsg(ModMsg),
    UpdateDone,
}
enum ThreadReply {
    Stopped,
    Error(Result<(), String>)
}

const DEF_FILE_NAME: &str = "mod_stats.txt";
const DEF_IDLE_SECS: u64 = 240;
const DEF_UPD_INTERVAL_SECS: u64 = 5;
thread_local! {
    static MOD_STATS: RefCell<ModStats>  = RefCell::new(ModStats::new());
    static UPD_INTERVAL: RefCell<Duration> = RefCell::new(Duration::from_secs(DEF_UPD_INTERVAL_SECS));
    static IDLE_NEW: RefCell<Duration> = RefCell::new(Duration::from_secs(DEF_IDLE_SECS));
    static MOD_STAT_FILE: RefCell<String> = RefCell::new(DEF_FILE_NAME.to_string());
}

fn reset() {
    MOD_STATS.with(|s| {
        let mut s = s.borrow_mut();
        s.last_render_update = SystemTime::now();
        s.last_rendered.clear();
        s.log_thread.as_mut().map(|lt| {
            if !lt.thread.is_finished() {
                let _ = lt.sender.send(ThreadCommand::Stop);
            }
        });
        s.log_thread = None;
    });
    UPD_INTERVAL.with(|s| {
        let mut s = s.borrow_mut();
        *s = Duration::from_secs(DEF_UPD_INTERVAL_SECS);
    });
    IDLE_NEW.with(|s| {
        let mut s = s.borrow_mut();
        *s = Duration::from_secs(DEF_IDLE_SECS);
    });
    MOD_STAT_FILE.with(|s| {
        let mut s = s.borrow_mut();
        *s = DEF_FILE_NAME.to_string();
    });
}

fn set_filename(filepath:&str) {
    MOD_STAT_FILE.with(|f| {
        let mut f = f.borrow_mut();
        *f = filepath.to_string();
    });
}

/// Process mod messages and update the mod stats file.
/// Updates existing lines, if found, for each mod in the list, provided the lines are within the
/// final 10K bytes of the file.  If not, new lines will be added.
///
/// This could be a lot simpler if it just slurped the entire file and rewrote it, but that would
/// be slower as log file gets big.
fn process_mod_msgs(msgs:&[ModMsg], filename:&str) -> Result<(), String> {
    use std::io::Write;

    if msgs.len() == 0 {
        return Ok(());
    }

    // if file doesn't exist create empty file
    if !std::path::Path::new(filename).exists() {
        std::fs::File::create(filename).map_err(|e| e.to_string())?;
    }

    // get file size of log
    let meta = std::fs::metadata(filename).map_err(|e| e.to_string())?;
    let file_size = meta.len();

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(filename)
        .map_err(|e| e.to_string())?;

    // if its less than 10K bytes just read it all in, otherwise read in the last 10K bytes
    let mut bytes_skipped = 0;
    if file_size > 10000 {
        file.seek(std::io::SeekFrom::End(-10000))
             .map_err(|e| e.to_string())?;
        bytes_skipped = file_size - 10000;
    };

    let mut buf = vec![];
    file.read_to_end(&mut buf).map_err(|e| e.to_string())?;

    //eprintln!("read {} bytes", buf.len());

    let mut patstrs = vec![];

    // build the list of patterns we are interested in
    for msg in msgs {
        if let ModMsg::ModActive(name, start_time, _) = msg {
            patstrs.push(format!("{}: '{}'", util::format_time(start_time), name));
        }
    }
    let patstrs = patstrs.iter().map(|s| s.as_bytes());
    let numpat = patstrs.len();
    let matches = util::aho_corasick_scan(patstrs, numpat, &buf);
    // find the lowest match, doesn't matter which pattern, use defv to indicate no matches
    let defv = file_size as usize + 1;
    let lowest = matches.iter().map(|v| {
        v.iter().map(|pair| pair.0).min().unwrap_or(defv)
    }).min().unwrap_or(defv);
    //eprintln!("{} searched, lowest match: {} (defv: {})", numpat, lowest, defv);

    // reset patterns including name of mod and updated time, and aggregate time for dupes
    let mut pats = HashMap::<String, Duration>::new();
    for msg in msgs {
        if let ModMsg::ModActive(name, start_time, time) = msg {
            let key = format!("{}: '{}'", util::format_time(start_time), name);
            let entry = pats.entry(key.clone()).or_insert(Duration::from_secs(0));
            *entry += *time;
        }
    }

    // make helper function to get an updated line for a mod
    let get_updated_line = |pat:&String, time:&Duration| {
        let timestr = if time.as_secs() >= 60 {
            format!("{}m {}s", time.as_secs() / 60, time.as_secs() % 60)
        } else {
            format!("{} secs", time.as_secs())
        };
        format!("{} {}", pat, timestr)
    };

    // reopen file for update (this assumes it hasn't changed and I am the only one writing it)
    drop(file);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        //.append(true)
        .read(true)
        .open(filename)
        .map_err(|e| e.to_string())?;

    // seek to the lowest match, taking into account any bytes skipped
    let seekoff = if lowest == defv {
        bytes_skipped + buf.len() as u64
    } else {
        lowest as u64 + bytes_skipped
    };

    let _pos = file.seek(std::io::SeekFrom::Start(seekoff))
        .map_err(|e| e.to_string())?;
    //eprintln!("seek {}, newpos: {}", seekoff, pos);

    if lowest == defv {
        // no matches, just write out new lines
        let lines = pats.into_iter().map(|(pat, time)| {
            get_updated_line(&pat, &time)
        });
        for line in lines {
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }
    } else {
        // build string starting at lowest, then change into lines
        let haystack = String::from_utf8_lossy(&buf[lowest..]);
        let lines = haystack.lines();
        let mut found_pats = HashSet::<String>::new();
        let lines = lines.map(|line| {
            // check for each pattern, this repeats some of the work done by corasick but avoids the
            // issue of how writing new lines would change the offsets it computed
            let found = pats.iter().find(|pat| line.starts_with(&*pat.0) );
            if let Some((pat, time)) = found {
                found_pats.insert(pat.clone());
                get_updated_line(pat, time)
            } else {
                // no match, just return the line
                line.to_string()
            }
        });
        // write out the updated lines
        for line in lines {
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }
        // and new pats we didn't find
        for (pat,time) in pats.iter() {
            if !found_pats.contains(pat) {
                writeln!(file, "{}", get_updated_line(pat, time)).map_err(|e| e.to_string())?;
            }
        }
    }
    // finally truncate to current position to avoid any stray bytes (which
    // can occur if we wrote shorter strings than was there before)
    file.flush().map_err(|e| e.to_string())?;

    let pos = file.seek(SeekFrom::Current(0))
        .map_err(|e| e.to_string())?;
    //eprintln!("set len to {}", pos);
    file.set_len(pos).map_err(|e| e.to_string())?;

    Ok(())
}

fn start_log_thread(filename:&str) -> LogThread {
    let (main_sender, thrd_receiver) = channel::<ThreadCommand>();
    let (thrd_sender, main_receiver) = channel::<ThreadReply>();
    let t_filename = filename.to_string();

    let logger = std::thread::spawn(move || {
        let mut err: Result<(), String> = Ok(());
        let mut msgs = vec![];
        loop {
            let cmd = thrd_receiver.recv();

            //eprintln!("recv msg");
            match cmd {
                Ok(cmd) => match cmd {
                    ThreadCommand::Stop => {
                        let _ = thrd_sender.send(ThreadReply::Stopped).unwrap_or(());
                        break;
                    }
                    ThreadCommand::ModMsg(msg) => {
                        msgs.push(msg);
                    },
                    ThreadCommand::UpdateDone => {
                        //eprintln!("processing {} msgs", msgs.len());
                        let res = process_mod_msgs(&msgs, &t_filename);
                        msgs.clear();
                        if res.is_err() {
                            thrd_sender.send(ThreadReply::Error(res)).unwrap_or(());
                        }
                    }
                },
                Err(e) => {
                    err = Err(e.to_string());
                    break;
                }
            }
        }

        if err.is_err() {
            thrd_sender.send(ThreadReply::Error(err)).unwrap_or(());
        }
    });

    LogThread {
        sender: main_sender,
        receiver: main_receiver,
        thread: logger,
    }

}

pub fn update(now:&SystemTime) -> Option<(u32,u32)> {
    MOD_STATS.with(|ms_rc| {
        let mut ms = ms_rc.borrow_mut();
        let mut elapsed = None;
        if !UPD_INTERVAL.with(|ui| {
            let ui = ui.borrow_mut();
            let mut delapsed = now.duration_since(ms.last_render_update).unwrap_or_else(|_x| Duration::from_secs(0));
            if delapsed < *ui {
                return false;
            }
            // limit extreme elapsed intervals (might mean we haven't updated in a while such
            // as when alt-tabbed away)
            if delapsed > (*ui * 2) {
                delapsed = *ui * 2;
            }
            elapsed = Some(delapsed);
            true
        }) {
            return None;
        }
        ms.last_render_update = *now;

        MOD_STAT_FILE.with(|f| {
            {
                let f = f.borrow();
                if *f != DEF_FILE_NAME {
                    return;
                }
            }

            // figure out filename
            let mm_root = match mm_verify_load() {
                Some(dir) => dir,
                None => {
                    write_log_file("mod_stats: no mm root found");
                    return;
                }
            };
            let basen = util::get_module_name_base().unwrap_or("".to_owned());
            if basen.is_empty() {
                return;
            }
            let mut ldir = mm_root.to_owned();
            ldir.push_str("\\Logs\\");
            let file_name = format!("modstats.{}.log", basen);
            ldir.push_str(&file_name);
            let mut f = f.borrow_mut();
            (*f) = ldir;
        });

        if ms.log_thread.is_none() {
            let filename = MOD_STAT_FILE.with(|f| f.borrow().to_owned());
            ms.log_thread = Some(start_log_thread(&filename));
        }

        let elapsed = elapsed.unwrap_or_else(|| Duration::from_secs(0));

        let (total_frames, loaded_mods) = unsafe {
            (GLOBAL_STATE.metrics.total_frames, GLOBAL_STATE.loaded_mods.as_ref())
        };

        let send_thread_cmd = |lt:&Option<LogThread>,cmd:ThreadCommand| {
            if let Some(log_thread) = lt {
                if log_thread.thread.is_finished() {
                    return;
                }
                let _ = log_thread.sender.send(cmd).map_err(|e| {
                    write_log_file(&format!("Error sending thread command: {}", e));
                });
            }
        };

        // descend into the insane loaded mods structure to find active mods
        let mut new_active = 0_u32;
        let mut total_active = 0_u32;
        loaded_mods
            .map(|m|  m.mods.values() )
            .map(|mlist| mlist.map(|m|
                m.iter().filter(|nmd| {
                    // if nmd.d3d_data.is_loaded() {
                    //     write_log_file(&format!("loaded mod: {}, last frame: {}, cur frame: {}",
                    //     nmd.name, nmd.last_frame_render, GLOBAL_STATE.metrics.total_frames ));
                    // }
                    nmd.d3d_data.is_loaded() && nmd.recently_rendered(total_frames)
                })))
            .map(|i| {
                for nmod in i {
                    nmod.for_each(|nmod| {
                        let idle_new = IDLE_NEW.with(|inew| *inew.borrow());
                        let modmsg;
                        match ms.last_rendered.get_mut(&nmod.name) {
                            None => {
                                ms.last_rendered.insert(nmod.name.clone(), (*now, *now, Duration::from_secs(0)));
                                new_active += 1;
                                total_active += 1;
                                modmsg = Some(ModMsg::NewModActive(nmod.name.clone(), *now));
                            },
                            // if it was idle for more than IDLE_NEW time, treat it as new
                            Some((_start, upd,  _dur))
                                if now.duration_since(*upd)
                                    .unwrap_or_else(|_| Duration::from_secs(0)) > idle_new => {
                                ms.last_rendered.insert(nmod.name.clone(), (*now, *now, Duration::from_secs(0)));
                                new_active += 1;
                                total_active += 1;
                                modmsg = Some(ModMsg::NewModActive(nmod.name.clone(), *now));
                            },
                            Some((start_time, upd, ref mut dur)) => {
                                total_active += 1;
                                *dur += elapsed;
                                *upd = *now;
                                modmsg = Some(ModMsg::ModActive(nmod.name.clone(), *start_time, *dur));
                            }
                        };
                        if let Some(msg) = modmsg {
                            send_thread_cmd(&ms.log_thread, ThreadCommand::ModMsg(msg));
                        }
                    });
                }
            });

        if total_active > 0 {
            send_thread_cmd(&ms.log_thread, ThreadCommand::UpdateDone);
        }

        // deal with complaints from log thread
        if let Some(ref log_thread) = &mut ms.log_thread {
            let mut dead = false;
            if let Ok(reply) = log_thread.receiver.try_recv() {
                match reply {
                    ThreadReply::Stopped => {
                        write_log_file("mod stats log thread stopped");
                        dead = true;
                    },
                    ThreadReply::Error(e) => {
                        write_log_file(&format!("Error from mod stats log thread: {:?}", e));
                    }
                }
            }
            if log_thread.thread.is_finished() {
                write_log_file("mod stats log thread finished");
                dead = true;
            }

            if dead {
                ms.log_thread = None;
            }
        }

        Some((new_active, total_active))
    })
}

#[cfg(test)]
mod tests {
    use fnv::FnvHashMap;
    use global_state::LoadedModState;
    use shared_dx::util::LOG_EXCL_LOCK;
    use types::{native_mod::{self, NativeModData, ModD3DState, ModD3DData, MAX_RECENT_RENDER_FRAMES}, interop::ModData, d3ddata::ModD3DData11};

    use super::*;
    use crate::hook_device_d3d11::tests::prep_log_file;
    use crate::util::format_time;

    fn set_update_interval_ms(ms:u64) {
        UPD_INTERVAL.with(|ui| {
            *ui.borrow_mut() = Duration::from_millis(ms);
        })
    }

    fn set_idle_new_ms(ms:u64) {
        IDLE_NEW.with(|inew| {
            *inew.borrow_mut() = Duration::from_millis(ms);
        })
    }

    #[test]
    fn test_process_messages_1() {
        let testfile = "__test_process_messages_1.txt";
        if std::path::Path::new(testfile).exists() {
            std::fs::remove_file(testfile).expect("doh");
        }

        let filedata = || -> String {
            let mut f = std::fs::File::open(testfile).expect("doh");
            let mut s = String::new();
            f.read_to_string(&mut s).expect("doh");
            s
        };

        let fooactive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata(), format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(10))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata(), format!("{}: 'foo' 10 secs\n", format_time(&fooactive)));
        let baractive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 10 secs", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),]);
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(15))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 15 secs", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),]);
        let crapactive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("crap".to_owned(), crapactive, Duration::from_secs(2))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 15 secs", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),
            format!("{}: 'crap' 2 secs", format_time(&crapactive)),]);
        let msgs = vec![
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(6)),
            ModMsg::ModActive("crap".to_owned(), crapactive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 15 secs", format_time(&fooactive)),
            format!("{}: 'bar' 6 secs", format_time(&baractive)),
            format!("{}: 'crap' 5 secs", format_time(&crapactive)),]);

        // don't need this, this test doesn't change the thread locals
        //super::reset();
    }

    #[test]
    fn test_process_messages_2() {
        let testfile = "__test_process_messages_2.txt";
        if std::path::Path::new(testfile).exists() {
            std::fs::remove_file(testfile).expect("doh");
        }

        let filedata = || -> String {
            let mut f = std::fs::File::open(testfile).expect("doh");
            let mut s = String::new();
            f.read_to_string(&mut s).expect("doh");
            s
        };
        let fooactive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");

        let append_str_rep = |s:&str, total_bytes_min:usize| {
            let craps_per_line = 10;
            let craps = total_bytes_min / s.len();
            let mut crapstr = String::new();
            for _line in 0..(craps/craps_per_line) {
                for _crap in 0..craps_per_line {
                    crapstr.push_str(s);
                }
                crapstr.push_str("\n");
            }
            let mut f = std::fs::OpenOptions::new().append(true).open(testfile).expect("doh");
            use std::io::Write;
            f.write_all(crapstr.as_bytes()).expect("doh");
            crapstr
        };

        // add crap to file
        let crapstr = append_str_rep("crap", 10000);

        // sleep a bit to get a different ts for bar
        std::thread::sleep(Duration::from_millis(1200));

        // now test appending after the crap
        let baractive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(10))];
        process_mod_msgs(&msgs, testfile).expect("doh");

        let fd = filedata();
        let mut ex = String::new();
        ex.push_str(&format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        ex.push_str(&crapstr);
        ex.push_str(&format!("{}: 'bar' 10 secs\n", format_time(&baractive)));
        assert!(fd == ex);

        // add more to bar
        let msgs = vec![
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(15))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        let fd = filedata();
        let mut ex = String::new();
        ex.push_str(&format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        ex.push_str(&crapstr);
        ex.push_str(&format!("{}: 'bar' 15 secs\n", format_time(&baractive)));
        assert!(fd == ex);

        // add more to foo, but because it is before the 10k window, this will result in a
        // new entry
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        let fd = filedata();
        let mut ex = String::new();
        ex.push_str(&format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        ex.push_str(&crapstr);
        ex.push_str(&format!("{}: 'bar' 15 secs\n", format_time(&baractive)));
        ex.push_str(&format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        assert!(fd == ex);

        // add a bit of dung
        let dungstr = append_str_rep("dung", 2000);
        // update bar & foo again (second entry)
        let catactive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(7)),
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(17)),
            ModMsg::ModActive("cat".to_owned(), catactive, Duration::from_secs(25))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        let fd = filedata();
        let mut ex = String::new();
        ex.push_str(&format!("{}: 'foo' 5 secs\n", format_time(&fooactive)));
        ex.push_str(&crapstr);
        ex.push_str(&format!("{}: 'bar' 17 secs\n", format_time(&baractive)));
        ex.push_str(&format!("{}: 'foo' 7 secs\n", format_time(&fooactive)));
        ex.push_str(&dungstr);
        ex.push_str(&format!("{}: 'cat' 25 secs\n", format_time(&catactive)));
        assert!(fd == ex);

        // don't need this, this test doesn't change the thread locals
        //super::reset();
    }

    #[test]
    fn test_process_messages_3() {
        let testfile = "__test_process_messages_3.txt";
        if std::path::Path::new(testfile).exists() {
            std::fs::remove_file(testfile).expect("doh");
        }

        let filedata = || -> String {
            let mut f = std::fs::File::open(testfile).expect("doh");
            let mut s = String::new();
            f.read_to_string(&mut s).expect("doh");
            s
        };

        let fooactive = SystemTime::now();
        let baractive = SystemTime::now();
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(10)),];
        process_mod_msgs(&msgs, testfile).expect("doh");
        let msgs = vec![
            ModMsg::ModActive("bar".to_owned(), baractive, Duration::from_secs(5))];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 10 secs", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),]);
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(59)),];
        process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 59 secs", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),]);
        let msgs = vec![
            ModMsg::ModActive("foo".to_owned(), fooactive, Duration::from_secs(60)),];
            process_mod_msgs(&msgs, testfile).expect("doh");
        assert_eq!(filedata().lines().collect::<Vec<&str>>(), vec![
            format!("{}: 'foo' 1m 0s", format_time(&fooactive)),
            format!("{}: 'bar' 5 secs", format_time(&baractive)),]);

        // don't need this, this test doesn't change the thread locals
        //super::reset();
    }

    #[test]
    fn test_mod_stats_update() {
        let _loglock = LOG_EXCL_LOCK.lock().unwrap();
        let _testlog = prep_log_file(&_loglock, "__test_mod_stats_update.txt").expect("doh");
        write_log_file("test starting");
        set_filename("__test_mod_stats.txt");
        set_update_interval_ms(0);
        assert_eq!(update(&SystemTime::now()), Some((0,0)));
        set_update_interval_ms(5);
        assert_eq!(update(&SystemTime::now()), None);
        std::thread::sleep(Duration::from_millis(6));
        assert_eq!(update(&SystemTime::now()), Some((0,0)));
        set_update_interval_ms(0);

        let mod_count = 5;
        // need to fill the global state with mods, which is tedious AF
        let mut loaded_mods: FnvHashMap<u32, Vec<native_mod::NativeModData>> =
            FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());
        let mut mods_by_name: FnvHashMap<String,u32> =
            FnvHashMap::with_capacity_and_hasher((mod_count * 10) as usize, Default::default());
        let mut midx = 0;
        let mut addmod = |name:&str,ref_prim:i32,ref_vert:i32| {
            let mod_name = name.to_owned();
            let mut nmd = NativeModData {
                midx: midx,
                mod_data: ModData::new(),
                name: mod_name.to_string(),
                last_frame_render: 0,
                d3d_data: ModD3DState::Unloaded,
                is_parent: false,
                parent_mod_names: vec![],
            };
            nmd.mod_data.numbers.ref_prim_count = ref_prim;
            nmd.mod_data.numbers.ref_vert_count = ref_vert;
            midx += 1;
            let mod_key = native_mod::NativeModData::mod_key(
                nmd.mod_data.numbers.ref_vert_count as u32,
                nmd.mod_data.numbers.ref_prim_count as u32,
            );
            mods_by_name.insert(mod_name, mod_key);
            loaded_mods.entry(mod_key).or_insert_with(|| vec![]).push(nmd);
        };

        let set_mod_rendered = |name:&str, idx:usize| {
            let frame = unsafe { GLOBAL_STATE.metrics.total_frames };
            let lms = unsafe { GLOBAL_STATE.loaded_mods.as_mut().unwrap() };
            let mod_key = lms.mods_by_name.get(name).expect("mod not found");
            let nmd = lms.mods.get_mut(mod_key).expect("mod not found").get_mut(idx).expect("mod not found");
            nmd.d3d_data = ModD3DState::Loaded(ModD3DData::D3D11(ModD3DData11::new()));
            nmd.last_frame_render = frame;
        };
        let advance_frames = |nframes:u64| {
            let frame = unsafe { GLOBAL_STATE.metrics.total_frames };
            unsafe { GLOBAL_STATE.metrics.total_frames = frame + nframes };
        };

        addmod("mod_100_200_1", 100, 200);
        addmod("mod_100_200_2", 100, 200); // variant
        addmod("mod_50_150", 50, 150);
        addmod("mod_10_20", 10, 20);
        addmod("mod_30_90", 30, 90);
        drop(addmod);

        let lms = LoadedModState {
            mods: loaded_mods,
            mods_by_name: mods_by_name,
            selected_variant: global_state::new_fnv_map(16),
        };
        unsafe { GLOBAL_STATE.loaded_mods = Some(lms); };
        set_update_interval_ms(0);
        assert_eq!(update(&SystemTime::now()), Some((0,0)));
        set_mod_rendered("mod_100_200_2", 1);
        assert_eq!(update(&SystemTime::now()), Some((1,1)));
        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(update(&SystemTime::now()), Some((0,1)));
        std::thread::sleep(Duration::from_millis(2));
        advance_frames(MAX_RECENT_RENDER_FRAMES+1);
        assert_eq!(update(&SystemTime::now()), Some((0,0)));
        set_mod_rendered("mod_100_200_2", 1);
        assert_eq!(update(&SystemTime::now()), Some((0,1)));
        set_idle_new_ms(5);
        std::thread::sleep(Duration::from_millis(6));
        assert_eq!(update(&SystemTime::now()), Some((1,1)));
        set_mod_rendered("mod_100_200_1", 0);
        assert_eq!(update(&SystemTime::now()), Some((1,2)));

        // now test that the log works
        set_mod_rendered("mod_50_150", 0);
        assert_eq!(update(&SystemTime::now()), Some((1,3)));
        assert_eq!(update(&SystemTime::now()), Some((0,3)));
        assert_eq!(update(&SystemTime::now()), Some((0,3)));

        std::thread::sleep(Duration::from_secs(1));

        unsafe { GLOBAL_STATE.loaded_mods = None };
        super::reset();
    }
}