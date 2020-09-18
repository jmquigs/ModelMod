use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::libloaderapi::GetModuleHandleA;
use winapi::um::winuser::{DefWindowProcA, DispatchMessageA, PeekMessageA, PostQuitMessage,
                          TranslateMessage, MSG, PM_REMOVE, WM_DESTROY};
use std;

lazy_static! {
    pub static ref TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

unsafe extern "system" fn test_wndproc(h: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        // TODO: set bool to exit d3d loop
        return 0;
    }

    DefWindowProcA(h, msg, w, l)
}

fn create_test_window() -> HWND {
    use std::ffi::CString;
    use std::ptr::null_mut;
    use winapi::um::winuser::*;

    let size = std::mem::size_of::<WNDCLASSEXA>() as u32;
    let title = CString::new("Direct3D Window").unwrap();
    let wndclass = WNDCLASSEXA {
        cbSize: size,
        style: CS_CLASSDC,
        lpfnWndProc: Some(test_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleA(null_mut()) },
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null_mut(),
        lpszClassName: title.as_ptr(),
        hIconSm: null_mut(),
    };

    unsafe { RegisterClassExA(&wndclass) };

    let t1 = CString::new("Direct3D Window").unwrap();
    let t2 = CString::new("ModelMod test").unwrap();

    let hwnd = unsafe {
        let desktop = GetDesktopWindow();
        let hwnd = CreateWindowExA(
            0,
            t1.as_ptr(),
            t2.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            640,
            480,
            desktop,
            null_mut(),
            wndclass.hInstance,
            null_mut(),
        );
        ShowWindow(hwnd, SW_SHOW);
        hwnd
    };

    hwnd
}

fn test_e2e() {
    let _lock = TEST_MUTEX.lock().unwrap();

    use std::ptr::null_mut;
    use std::time::SystemTime;
    use winapi::shared::d3d9::*;
    use winapi::shared::d3d9types::*;

    let d3d9 = crate::hook_device::create_d3d9(32).expect("failed to create d3d9");
    // create a device
    let hwnd = create_test_window();

    let mut pp: D3DPRESENT_PARAMETERS = unsafe { std::mem::zeroed() };

    //println!("{} {} {} {}", pp.BackBufferWidth, pp.BackBufferHeight, pp.BackBufferFormat, pp.BackBufferCount);
    pp.Windowed = 1;
    pp.SwapEffect = D3DSWAPEFFECT_DISCARD;
    pp.BackBufferFormat = D3DFMT_UNKNOWN;

    let mut pDevice: *mut IDirect3DDevice9 = std::ptr::null_mut();
    let hr = unsafe {
        (*d3d9).CreateDevice(
            D3DADAPTER_DEFAULT,
            D3DDEVTYPE_HAL,
            hwnd,
            D3DCREATE_HARDWARE_VERTEXPROCESSING,
            &mut pp,
            &mut pDevice,
        )
    };
    assert_eq!(hr, 0, "failed to create device");

    unsafe {
        let mut msg: MSG = std::mem::zeroed();

        // TODO: don't hardcode; stop when tests complete

        let start = SystemTime::now();
        loop {
            let now = SystemTime::now();
            let elapsed = now.duration_since(start);
            if elapsed.unwrap().as_secs() > 25 {
                // TODO let test signal end
                break;
            }

            (*pDevice).BeginScene();

            // TODO: DIP

            (*pDevice).EndScene();

            (*pDevice).Present(null_mut(), null_mut(), null_mut(), null_mut());

            TranslateMessage(&msg);
            DispatchMessageA(&msg);
            PeekMessageA(&mut msg, null_mut(), 0, 0, PM_REMOVE);

            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    unsafe {
        (*pDevice).Release();
        (*d3d9).Release();
    };
}

#[test]
fn _test_all() {
    // TODO: ultimately will want to be able to run this multiple times and have it produce
    // same effect each time. will need to clear some accumulated state, reset clr, after each run
    // to do that
    //for _i in 0..2 {
    test_e2e();
    //}
}
