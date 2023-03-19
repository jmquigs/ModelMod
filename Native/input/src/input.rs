#![allow(non_snake_case)]
//use winapi::shared::d3d9::*;
//use winapi::shared::d3d9types::*;
use winapi::shared::minwindef::*;
//use winapi::shared::windef::{HWND, RECT};
use winapi::ctypes::c_void;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::dinput::{GUID_Key, GUID_SysKeyboard, IID_IDirectInput8W};
use winapi::um::unknwnbase::LPUNKNOWN;
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winnt::HRESULT;

//use winapi::shared::winerror::{S_OK};
use winapi::shared::guiddef::{GUID, REFGUID, REFIID};
// use winapi::ctypes::c_void;
// use winapi::um::wingdi::RGNDATA;

use fnv::FnvHashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

//extern HRESULT WINAPI DirectInput8Create(HINSTANCE hinst, DWORD dwVersion, REFIID riidltf, LPVOID *ppvOut, LPUNKNOWN punkOuter);

use shared_dx::error::*;
use shared_dx::util::write_log_file;


use std::ptr::null_mut;

use profiler::*;

//use profile::*;

#[repr(C)]
pub struct DIOBJECTDATAFORMAT(*const GUID, DWORD, DWORD, DWORD);

#[repr(C)]
pub struct DIDATAFORMAT {
    dwSize: DWORD,
    dwObjSize: DWORD,
    dwFlags: DWORD,
    dwDataSize: DWORD,
    dwNumObjs: DWORD,
    rgodf: *const DIOBJECTDATAFORMAT,
}

RIDL!(#[uuid(0x54d41081, 0xdc15, 0x4833, 0xa4, 0x1b, 0x74, 0x8f, 0x73, 0xa3, 0x81, 0x79)]
interface IDirectInputDevice8W(IDirectInputDevice8WVtbl): IUnknown(IUnknownVtbl) {
    fn GetCapabilities(caps:LPVOID /*LPDIDEVCAPS*/,) -> HRESULT,
    fn EnumObjects(cb:LPVOID /*LPDIENUMDEVICEOBJECTSCALLBACKW*/,obj:LPVOID,dw:DWORD,) -> HRESULT,
    fn GetProperty(d:REFGUID,hdr:LPVOID /*LPDIPROPHEADER*/,) -> HRESULT,
    fn SetProperty(d:REFGUID,hdr:LPVOID /*LPCDIPROPHEADER*/,) -> HRESULT,
    fn Acquire() -> HRESULT,
    fn Unacquire() -> HRESULT,
    fn GetDeviceState(dw:DWORD,put:LPVOID,) -> HRESULT,
    fn GetDeviceData(cbObjectData:DWORD,rgdod:LPVOID /*LPDIDEVICEOBJECTDATA*/,
        pdwInOut:LPDWORD,dwFlags:DWORD,) -> HRESULT,
    fn SetDataFormat(df:*mut DIDATAFORMAT,) -> HRESULT,
    // fn SetEventNotification(HANDLE) -> HRESULT,
    // fn SetCooperativeLevel(HWND,DWORD) -> HRESULT,
    // fn GetObjectInfo(LPDIDEVICEOBJECTINSTANCEW,DWORD,DWORD) -> HRESULT,
    // fn GetDeviceInfo(LPDIDEVICEINSTANCEW) -> HRESULT,
    // fn RunControlPanel(HWND,DWORD) -> HRESULT,
    // fn Initialize(HINSTANCE,DWORD,REFGUID) -> HRESULT,
    // fn CreateEffect(REFGUID,LPCDIEFFECT,LPDIRECTINPUTEFFECT *,LPUNKNOWN) -> HRESULT,
    // fn EnumEffects(LPDIENUMEFFECTSCALLBACKW,LPVOID,DWORD) -> HRESULT,
    // fn GetEffectInfo(LPDIEFFECTINFOW,REFGUID) -> HRESULT,
    // fn GetForceFeedbackState(LPDWORD) -> HRESULT,
    // fn SendForceFeedbackCommand(DWORD) -> HRESULT,
    // fn EnumCreatedEffectObjects(LPDIENUMCREATEDEFFECTOBJECTSCALLBACK,LPVOID,DWORD) -> HRESULT,
    // fn Escape(LPDIEFFESCAPE) -> HRESULT,
    // fn Poll(THIS) -> HRESULT,
    // fn SendDeviceData(DWORD,LPCDIDEVICEOBJECTDATA,LPDWORD,DWORD) -> HRESULT,
    // fn EnumEffectsInFile(LPCWSTR,LPDIENUMEFFECTSINFILECALLBACK,LPVOID,DWORD) -> HRESULT,
    // fn WriteEffectToFile(LPCWSTR,DWORD,LPDIFILEEFFECT,DWORD) -> HRESULT,
    // fn BuildActionMap(LPDIACTIONFORMATW,LPCWSTR,DWORD) -> HRESULT,
    // fn SetActionMap(LPDIACTIONFORMATW,LPCWSTR,DWORD) -> HRESULT,
    // fn GetImageInfo(LPDIDEVICEIMAGEINFOHEADERW) -> HRESULT,
});

RIDL!(#[uuid(0xbf798031, 0x483a, 0x4da2, 0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00)]
interface IDirectInput8W(IDirectInput8WVtbl): IUnknown(IUnknownVtbl) {
    fn CreateDevice(dev:REFGUID, outdev:*mut *mut IDirectInputDevice8W, something: LPUNKNOWN,)
        -> HRESULT,
    // fn(EnumDevices(DWORD,LPDIENUMDEVICESCALLBACKW,LPVOID,DWORD) -> HRESULT,
    // fn(GetDeviceStatus(REFGUID) -> HRESULT,
    // fn(RunControlPanel(HWND,DWORD) -> HRESULT,
    // fn(Initialize(HINSTANCE,DWORD) -> HRESULT,
    // fn(FindDevice(REFGUID,LPCWSTR,LPGUID) -> HRESULT,
    // fn(EnumDevicesBySemantics(LPCWSTR,LPDIACTIONFORMATW,LPDIENUMDEVICESBYSEMANTICSCBW,LPVOID,DWORD) -> HRESULT,
    // fn(ConfigureDevices(LPDICONFIGUREDEVICESCALLBACK,LPDICONFIGUREDEVICESPARAMSW,DWORD,LPVOID) -> HRESULT,
});

const DIRECTINPUT_VERSION: DWORD = 0x0800;
type DirectInput8CreateFn = unsafe extern "system" fn(
    hinst: HINSTANCE,
    dwVersion: DWORD,
    riidltf: REFIID,
    ppvOut: *mut LPVOID,
    punkOuter: LPUNKNOWN,
) -> HRESULT;

const INITIAL_REPEAT_DELAY: u16 = 500;
const CONTINUED_REPEAT_DELAY: u16 = 75;

pub const DIK_LALT: u8 = 0x38;
pub const DIK_RALT: u8 = 0xB8;
pub const DIK_LSHIFT: u8 = 0x2A;
pub const DIK_RSHIFT: u8 = 0x36;
pub const DIK_LCONTROL: u8 = 0x1D;
pub const DIK_RCONTROL: u8 = 0x9D;
pub const DIK_F1: u8 = 0x3B;
pub const DIK_F2: u8 = 0x3C;
pub const DIK_F3: u8 = 0x3D;
pub const DIK_F4: u8 = 0x3E;
pub const DIK_F5: u8 = 0x3F;
pub const DIK_F6: u8 = 0x40;
pub const DIK_F7: u8 = 0x41;
pub const DIK_F8: u8 = 0x42;
pub const DIK_F9: u8 = 0x43;
pub const DIK_F10: u8 = 0x44;
pub const DIK_NUMPAD8: u8 = 0x48;
pub const DIK_NUMPAD9: u8 = 0x49;

pub const DIK_BACKSLASH: u8 = 0x2B;
pub const DIK_RBRACKET: u8 = 0x1B;
pub const DIK_SEMICOLON: u8 = 0x27;
pub const DIK_COMMA: u8 = 0x33;
pub const DIK_PERIOD: u8 = 0x34;
pub const DIK_SLASH: u8 = 0x35;

#[derive(Debug)]
pub struct KeyEvent {
    pub key: u8,
    pub pressed: bool,
}

pub struct Input {
    events: Vec<KeyEvent>,
    keyboard_state: Vec<u8>,
    last_keyboard_state: Vec<u8>,
    last_press_event: Vec<SystemTime>,
    last_update: SystemTime,
    press_event_fns: FnvHashMap<u8, Box<dyn FnMut()>>,
    repeat_delay: Vec<u16>,
    pub alt_pressed: bool,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
    pub setup_attempts: i32,
    keyboard: *mut IDirectInputDevice8W,
}

decl_profile_globals!(inp);

impl Input {
    pub fn new() -> Result<Self> {
        let mut inp = Input {
            events: Vec::new(),
            keyboard_state: Vec::with_capacity(256),
            last_keyboard_state: Vec::with_capacity(256),
            repeat_delay: Vec::new(),
            last_press_event: Vec::new(),
            last_update: SystemTime::now(),
            press_event_fns: FnvHashMap::with_capacity_and_hasher(
                1024_usize,
                Default::default(),
            ),
            alt_pressed: false,
            shift_pressed: false,
            ctrl_pressed: false,
            keyboard: null_mut(),
            setup_attempts: 0
        };
        for _i in 0..256 {
            inp.keyboard_state.push(0);
            inp.last_keyboard_state.push(0);
            inp.repeat_delay.push(0);
            inp.last_press_event.push(UNIX_EPOCH);
        }
        unsafe {
            inp.keyboard = inp.init()?;
        };
        Ok(inp)
    }
    unsafe fn init(&mut self) -> Result<*mut IDirectInputDevice8W> {
        use winapi::um::libloaderapi::*;

        let lib = util::load_lib("dinput8.dll")?;
        let create = util::get_proc_address(lib, "DirectInput8Create")?;

        let mut dinput8: *mut c_void = null_mut();
        let create: DirectInput8CreateFn = std::mem::transmute(create);
        let handle = GetModuleHandleW(null_mut());
        let hr = (create)(
            handle,
            DIRECTINPUT_VERSION,
            &IID_IDirectInput8W,
            &mut dinput8,
            null_mut(),
        );
        if hr != 0 {
            return Err(HookError::DInputCreateFailed(format!(
                "failed to create dinput8: {:x}",
                hr
            )));
        }
        let dinput8: *mut IDirectInput8W = std::mem::transmute(dinput8);

        let mut keyboard: *mut IDirectInputDevice8W = null_mut();
        let hr = (*dinput8).CreateDevice(&GUID_SysKeyboard, &mut keyboard, null_mut());
        if hr != 0 {
            return Err(HookError::DInputCreateFailed(format!(
                "failed to create dinput keyboard: {:x}",
                hr
            )));
        }

        let arr_ptr = &DF_DIKEYBOARD[0] as *const DIOBJECTDATAFORMAT;

        let mut c_dfDIKeyboard: DIDATAFORMAT = DIDATAFORMAT {
            dwSize: std::mem::size_of::<DIDATAFORMAT>() as DWORD,
            dwObjSize: std::mem::size_of::<DIOBJECTDATAFORMAT>() as DWORD,
            dwFlags: 1, /*DIDF_RELAXIS*/
            dwDataSize: 256,
            dwNumObjs: 256, /*numObjects(dfDIKeyboard)*/
            rgodf: arr_ptr, /* (LPDIOBJECTDATAFORMAT)dfDIKeyboard */
        };

        let hr = (*keyboard).SetDataFormat(&mut c_dfDIKeyboard);
        if hr != 0 {
            return Err(HookError::DInputCreateFailed(format!(
                "failed to set data format on keyboard: {:x}",
                hr
            )));
        }
        let hr = (*keyboard).Acquire();
        if hr != 0 {
            return Err(HookError::DInputCreateFailed(format!(
                "failed to acquire keyboard: {:x}",
                hr
            )));
        }

        write_log_file("created dinput keyboard");

        Ok(keyboard)
    }

    pub fn clear_handlers(&mut self) {
        self.press_event_fns.clear();
    }

    pub fn add_press_fn(&mut self, key: u8, fun: Box<dyn FnMut()>) {
        self.press_event_fns.insert(key, fun);
    }
    pub fn get_press_fn_count(&self) -> usize {
        self.press_event_fns.len()
    }

    pub fn events(&self) -> &Vec<KeyEvent> {
        &self.events
    }

    pub fn process(&mut self) -> Result<()> {
        profile_start!(inp, check);

        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_update)?;
        let ms = elapsed.subsec_nanos() as f64 * 1e-6;
        if ms < 16.0 {
            profile_end!(inp, check);
            return Ok(());
        }
        profile_end!(inp, check);
        profile_start!(inp, process);

        self.events.clear();
        self.last_update = now;

        // TODO: need to clear before GetDeviceState?
        unsafe {
            {
                let mut gds = |acquire| {
                    if acquire {
                        let hr = (*self.keyboard).Acquire();
                        if hr != 0 {
                            return hr;
                        }
                    }
                    (*self.keyboard).GetDeviceState(
                        (std::mem::size_of::<i8>() * 256) as u32,
                        std::mem::transmute(self.keyboard_state.as_mut_ptr()),
                    )
                };

                let mut hr = gds(false);
                if hr != 0 {
                    // reacquire
                    hr = gds(true);

                    if hr != 0 {
                        return Err(HookError::DInputError(format!(
                            "failed to reacquire keyboard for input: {:x}",
                            hr
                        )));
                    }
                }
            }

            // update modifiers
            self.alt_pressed = (self.keyboard_state[DIK_LALT as usize] & 0x80) > 0
                || (self.keyboard_state[DIK_RALT as usize] & 0x80) > 0;
            self.shift_pressed = (self.keyboard_state[DIK_LSHIFT as usize] & 0x80) > 0
                || (self.keyboard_state[DIK_RSHIFT as usize] & 0x80) > 0;
            self.ctrl_pressed = (self.keyboard_state[DIK_LCONTROL as usize] & 0x80) > 0
                || (self.keyboard_state[DIK_RCONTROL as usize] & 0x80) > 0;

            let zero_ms = Duration::from_millis(0);

            for i in 0..256 {
                let i = i as usize;
                let pressed = self.keyboard_state[i] & 0x80 > 0;
                let was_pressed = self.last_keyboard_state[i] & 0x80 > 0;
                let new_press = pressed && !was_pressed;
                let new_release = !pressed && was_pressed;

                if new_press {
                    // per-key repeat delay is probably overkill, but who cares.
                    self.repeat_delay[i] = INITIAL_REPEAT_DELAY;
                }

                let repeat = !new_press && pressed && self.last_press_event[i] != UNIX_EPOCH
                    && (now.duration_since(self.last_press_event[i])
                        .unwrap_or(zero_ms)
                        .subsec_nanos() as f64 * 1e-6)
                        >= self.repeat_delay[i].into();
                if repeat {
                    // switch to lower delay now
                    self.repeat_delay[i] = CONTINUED_REPEAT_DELAY;
                }

                if new_press || repeat {
                    self.events.push(KeyEvent {
                        key: i as u8,
                        pressed: true,
                    });
                    self.last_press_event[i] = now;
                } else if new_release {
                    self.events.push(KeyEvent {
                        key: i as u8,
                        pressed: false,
                    });
                }

                self.last_keyboard_state[i] = self.keyboard_state[i];
            }
        };

        for evt in self.events.iter() {
            //write_log_file(&format!("event: {:x} pressed: {}", ke.key, ke.pressed));
            if evt.pressed && self.ctrl_pressed {
                if let Some(fun) = self.press_event_fns.get_mut(&evt.key) { fun(); }
            }
        }
        // if self.events.len() > 0 {
        //     write_log_file("");
        // }

        profile_end!(inp, process);

        profile_summarize!(inp, 10.0);

        Ok(())
    }
}

const DIDFT_OPTIONAL: DWORD = 0x80000000;
const DIDFT_BUTTON: DWORD = 0x0000000C;
//#define DIDFT_MAKEINSTANCE(n) ((WORD)(n) << 8)
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_input() {
        println!(
            "sizeof DIOBJECTDATAFORMAT: {}",
            std::mem::size_of::<DIOBJECTDATAFORMAT>()
        );
        println!(
            "sizeof DIDATAFORMAT: {}",
            std::mem::size_of::<DIDATAFORMAT>()
        );
        //println!("c_dfDIKeyboard: {:?}", DF_DIKEYBOARD);
        //println!("sizeof c_dfDIKeyboard: {}", std::mem::size_of::<c_dfDIKeyboard>());
    }
}

macro_rules! keyformat {
    ($x:expr) => {
        DIOBJECTDATAFORMAT(
            &GUID_Key,
            $x,
            DIDFT_OPTIONAL | DIDFT_BUTTON | (($x as u16) << 8) as DWORD,
            0,
        )
    };
}

const DF_DIKEYBOARD: [DIOBJECTDATAFORMAT; 256] = [
    keyformat!(0),
    keyformat!(1),
    keyformat!(2),
    keyformat!(3),
    keyformat!(4),
    keyformat!(5),
    keyformat!(6),
    keyformat!(7),
    keyformat!(8),
    keyformat!(9),
    keyformat!(10),
    keyformat!(11),
    keyformat!(12),
    keyformat!(13),
    keyformat!(14),
    keyformat!(15),
    keyformat!(16),
    keyformat!(17),
    keyformat!(18),
    keyformat!(19),
    keyformat!(20),
    keyformat!(21),
    keyformat!(22),
    keyformat!(23),
    keyformat!(24),
    keyformat!(25),
    keyformat!(26),
    keyformat!(27),
    keyformat!(28),
    keyformat!(29),
    keyformat!(30),
    keyformat!(31),
    keyformat!(32),
    keyformat!(33),
    keyformat!(34),
    keyformat!(35),
    keyformat!(36),
    keyformat!(37),
    keyformat!(38),
    keyformat!(39),
    keyformat!(40),
    keyformat!(41),
    keyformat!(42),
    keyformat!(43),
    keyformat!(44),
    keyformat!(45),
    keyformat!(46),
    keyformat!(47),
    keyformat!(48),
    keyformat!(49),
    keyformat!(50),
    keyformat!(51),
    keyformat!(52),
    keyformat!(53),
    keyformat!(54),
    keyformat!(55),
    keyformat!(56),
    keyformat!(57),
    keyformat!(58),
    keyformat!(59),
    keyformat!(60),
    keyformat!(61),
    keyformat!(62),
    keyformat!(63),
    keyformat!(64),
    keyformat!(65),
    keyformat!(66),
    keyformat!(67),
    keyformat!(68),
    keyformat!(69),
    keyformat!(70),
    keyformat!(71),
    keyformat!(72),
    keyformat!(73),
    keyformat!(74),
    keyformat!(75),
    keyformat!(76),
    keyformat!(77),
    keyformat!(78),
    keyformat!(79),
    keyformat!(80),
    keyformat!(81),
    keyformat!(82),
    keyformat!(83),
    keyformat!(84),
    keyformat!(85),
    keyformat!(86),
    keyformat!(87),
    keyformat!(88),
    keyformat!(89),
    keyformat!(90),
    keyformat!(91),
    keyformat!(92),
    keyformat!(93),
    keyformat!(94),
    keyformat!(95),
    keyformat!(96),
    keyformat!(97),
    keyformat!(98),
    keyformat!(99),
    keyformat!(100),
    keyformat!(101),
    keyformat!(102),
    keyformat!(103),
    keyformat!(104),
    keyformat!(105),
    keyformat!(106),
    keyformat!(107),
    keyformat!(108),
    keyformat!(109),
    keyformat!(110),
    keyformat!(111),
    keyformat!(112),
    keyformat!(113),
    keyformat!(114),
    keyformat!(115),
    keyformat!(116),
    keyformat!(117),
    keyformat!(118),
    keyformat!(119),
    keyformat!(120),
    keyformat!(121),
    keyformat!(122),
    keyformat!(123),
    keyformat!(124),
    keyformat!(125),
    keyformat!(126),
    keyformat!(127),
    keyformat!(128),
    keyformat!(129),
    keyformat!(130),
    keyformat!(131),
    keyformat!(132),
    keyformat!(133),
    keyformat!(134),
    keyformat!(135),
    keyformat!(136),
    keyformat!(137),
    keyformat!(138),
    keyformat!(139),
    keyformat!(140),
    keyformat!(141),
    keyformat!(142),
    keyformat!(143),
    keyformat!(144),
    keyformat!(145),
    keyformat!(146),
    keyformat!(147),
    keyformat!(148),
    keyformat!(149),
    keyformat!(150),
    keyformat!(151),
    keyformat!(152),
    keyformat!(153),
    keyformat!(154),
    keyformat!(155),
    keyformat!(156),
    keyformat!(157),
    keyformat!(158),
    keyformat!(159),
    keyformat!(160),
    keyformat!(161),
    keyformat!(162),
    keyformat!(163),
    keyformat!(164),
    keyformat!(165),
    keyformat!(166),
    keyformat!(167),
    keyformat!(168),
    keyformat!(169),
    keyformat!(170),
    keyformat!(171),
    keyformat!(172),
    keyformat!(173),
    keyformat!(174),
    keyformat!(175),
    keyformat!(176),
    keyformat!(177),
    keyformat!(178),
    keyformat!(179),
    keyformat!(180),
    keyformat!(181),
    keyformat!(182),
    keyformat!(183),
    keyformat!(184),
    keyformat!(185),
    keyformat!(186),
    keyformat!(187),
    keyformat!(188),
    keyformat!(189),
    keyformat!(190),
    keyformat!(191),
    keyformat!(192),
    keyformat!(193),
    keyformat!(194),
    keyformat!(195),
    keyformat!(196),
    keyformat!(197),
    keyformat!(198),
    keyformat!(199),
    keyformat!(200),
    keyformat!(201),
    keyformat!(202),
    keyformat!(203),
    keyformat!(204),
    keyformat!(205),
    keyformat!(206),
    keyformat!(207),
    keyformat!(208),
    keyformat!(209),
    keyformat!(210),
    keyformat!(211),
    keyformat!(212),
    keyformat!(213),
    keyformat!(214),
    keyformat!(215),
    keyformat!(216),
    keyformat!(217),
    keyformat!(218),
    keyformat!(219),
    keyformat!(220),
    keyformat!(221),
    keyformat!(222),
    keyformat!(223),
    keyformat!(224),
    keyformat!(225),
    keyformat!(226),
    keyformat!(227),
    keyformat!(228),
    keyformat!(229),
    keyformat!(230),
    keyformat!(231),
    keyformat!(232),
    keyformat!(233),
    keyformat!(234),
    keyformat!(235),
    keyformat!(236),
    keyformat!(237),
    keyformat!(238),
    keyformat!(239),
    keyformat!(240),
    keyformat!(241),
    keyformat!(242),
    keyformat!(243),
    keyformat!(244),
    keyformat!(245),
    keyformat!(246),
    keyformat!(247),
    keyformat!(248),
    keyformat!(249),
    keyformat!(250),
    keyformat!(251),
    keyformat!(252),
    keyformat!(253),
    keyformat!(254),
    keyformat!(255),
];
