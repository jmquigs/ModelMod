#![allow(non_snake_case)]

/*
TL;DR run modes:

# repeatedly test geometry with the specified prim,vert count.  MM if hooked will eventually
# render a mod it its place, if it has one.  It can take 10-15 seconds or more before this 
# happens during which time the window will most likely be blank.
# you'll probably need to make a MM profile for this to work, read the long text below for more info.
sh runmm.sh 9303,6534

# render a spinning cube, doesn't do much else. the prim vert/count here is ignored.
sh runmm.sh 100,200 -shape

Long description:

This standalone program allows testing of the entire code base without injecting into a
real game exe.  This program acts like a d3d11 game, making appropriate calls to render meshes,
although the meshes it renders are (usually) empty/garbage; MM is hooked into the process
(via the `runmm.sh` script), so it can intercept and perform actions on these rendering calls
like it normally would.

The program has two major run modes:
- if no -shape option is present, the program generates a placeholder index/vertex buffer for 
the specify primitive vertex count and repeatedly renders it, the idea being MM will observe that
and try to look up a mod (and render it if it has one)
- if  -shape option is present, it renders a hardcoded test shape (a spinning cube). this is mostly 
here to test this program's ability to render with d3d11 independent of MM.  While prim/vert counts,
can still be specified, since the program is rendering cube verts, MM most likely won't ever see a 
matching mod and won't render or even load much of anything.

If rendering a mod, usually it will render flat-shaded (albeit with some semi-broken lighting effects
I added), not with a texture, because this program doesn't set textures by default and neither does MM unless the 
mod uses override textures, which most don't (also I haven't updated the pixel shader to sample textures).
The -tex parameter can be used to specify a path to a texture that this program will load and make 
available to the shaders.

The typical usage for this is:
1) run `cargo run` to build and run the program
2) create a MM launch profile for the resulting `./target/debug/test_native_launch.exe`, in the
game profile for that exe, specify the MM data path that contains the data you want to load
(this can be the data path for a real game)
3) run this program (using git bash) with an argument that specifies the "mod" geometry it will draw.
For instance `sh ./runmm.sh 1500,1200` will cause this program to (peridiocally) issue a draw call
for that prim and vert count, and if a mod is available that will trigger the mod loading process
for it.

This program doesn't communicate with MM, just like a normal game wouldn't, so after loading it
typically doesn't produce much useful output.  But the output of MM can be observed by examining
the log file, typically `$mmlogdir\Logs\ModelMod.test_native_launch.log`.  It may also be possible
to attach to this process via the managed code debugger, but I haven't tried that.

A fair portion of this code was written by github copilot and other LLMs.  And I've changed what 
I wanted it to do a few times without cleaning it up much.  So, it's not very good.  But it's
good enough to test the mod loading process.  

Vertex Layout notes:
A vertex layout must be specified to render anything.  "basic_format.txt" contains a layout 
that works for the test shape and for simplistic mod rendering.  If you want some other vertex 
format you must (at least) create a new format .txt file and a new vertex shader for it,
as the vertex shader is directly tied to the format (using a struct like VS_INPUT).  You may 
be able to reuse the shape pixel shader.

When creating a new format, pay careful attention to the vertex size and the byte alignment
values and format types in the input layout description.  These should match what the
game is reporting for the mod you are interested in.  Be careful that your offsets and sizes 
don't violate any byte alignment restrictions, and that the formats used match what the 
shader expects.

Note, layout description arrays cannot be simply captured from the game like the shaders can,
because they contain a raw ascii pointer to the semantic name, which will crash here if you
try to use it.  This is one reason why this program uses a text file to specify the format, 
another is that its much easier to change that than a binary file.

*/

use std::ptr::null_mut;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{path::PathBuf, time::SystemTime};

use glam::{Vec2, Vec3};
// If these imports get too ugly or rust analyzer sticks them all on one line, use LLM to reorg them.
use winapi::ctypes::c_void;
use winapi::shared::dxgiformat::DXGI_FORMAT_D24_UNORM_S8_UINT;
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::shared::winerror::SUCCEEDED;
use winapi::shared::{
    dxgi::{
        IDXGIAdapter, IDXGISwapChain, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH,
        DXGI_SWAP_EFFECT_DISCARD,
    },
    dxgiformat::{
        DXGI_FORMAT_R16_UINT, DXGI_FORMAT_R8G8B8A8_UNORM,
    },
    dxgitype::{
        DXGI_MODE_DESC, DXGI_MODE_SCALING_UNSPECIFIED, DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
        DXGI_RATIONAL, DXGI_SAMPLE_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    },
            minwindef::{HMODULE, LPARAM, LPVOID, LRESULT, UINT, WPARAM}, 
    ntdef::{HRESULT, LPCSTR, LPCWSTR},
    windef::{HBRUSH, HCURSOR, HICON, HMENU, HWND},
    winerror::DXGI_ERROR_SDK_COMPONENT_MISSING,
};
use winapi::um::d3d11::{ID3D11Asynchronous, ID3D11DepthStencilView, ID3D11PixelShader, ID3D11Query, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D, D3D11_BIND_DEPTH_STENCIL, D3D11_CLEAR_DEPTH, D3D11_CLEAR_STENCIL, D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_CULL_FRONT, D3D11_FILL_SOLID, D3D11_QUERY, D3D11_QUERY_DATA_PIPELINE_STATISTICS, D3D11_QUERY_DESC, D3D11_QUERY_PIPELINE_STATISTICS, D3D11_RASTERIZER_DESC, D3D11_TEXTURE2D_DESC};
use winapi::um::d3d11sdklayers::{ID3D11InfoQueue, IID_ID3D11InfoQueue, D3D11_MESSAGE_SEVERITY_CORRUPTION, D3D11_MESSAGE_SEVERITY_ERROR, D3D11_MESSAGE_SEVERITY_INFO, D3D11_MESSAGE_SEVERITY_WARNING};
use winapi::um::libloaderapi::{LoadLibraryExW, LoadLibraryW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use winapi::um::{
    d3d11::{
        ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout, ID3D11VertexShader,
                D3D11_BIND_INDEX_BUFFER, D3D11_BIND_VERTEX_BUFFER, D3D11_BUFFER_DESC, 
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG, D3D11_INPUT_ELEMENT_DESC, 
        D3D11_SDK_VERSION, D3D11_SUBRESOURCE_DATA, D3D11_USAGE_DEFAULT,
    },
    d3dcommon::{D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0},
    libloaderapi::{GetModuleHandleA, GetProcAddress},
    winuser::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, PostQuitMessage,
        RegisterClassExW, ShowWindow, TranslateMessage, COLOR_WINDOWFRAME, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, PM_REMOVE, SW_SHOWDEFAULT, WM_CLOSE, WM_DESTROY, WM_QUIT, WNDCLASSEXW,
        WS_OVERLAPPEDWINDOW,
    },
};

use winapi::um::d3dcommon::{D3D_DRIVER_TYPE,D3D_FEATURE_LEVEL};

use winapi::um::errhandlingapi::GetLastError;
use winapi::Interface;

use crate::load_mmobj::test_load_mmobj;
use crate::render::{get_empty_vertices, get_indices, prepare_shader_constants, ModelViewParams};

#[macro_use]
extern crate anyhow;

mod load_mmobj;
mod interop_mmobj;
mod shadercomp;
mod shape;
mod render;
mod d3d11_utilfn;
mod util;


static WINEVENTS: Mutex<Vec<WinEvent>> = Mutex::new(vec![]);
fn add_winevent(evt:WinEvent) {
    {
        match WINEVENTS.lock() {
            Ok(mut lck) => {
                lck.push(evt);
            },
            Err(e) => eprintln!("failed to lock winevents: {:?}", e)
        }
    }
}


enum WinEvent {
    MouseWheel(i16),
    MousePan(i16, i16),
    MouseRot(i16, i16),
}

fn get_x_lparam(lparam: LPARAM) -> i16 {
    (lparam & 0xFFFF) as i16 
}

fn get_y_lparam(lparam: LPARAM) -> i16 {
    ((lparam >> 16) & 0xFFFF) as i16
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // println!("wnd proc: {:x}", msg);

    static mut LAST_MOUSE_POS: (i16, i16) = (0, 0);
    static mut MOUSE_PAN_ACTIVE: bool = false;
    static mut MOUSE_MOVE_WITHOUT_SHIFT_ACTIVE: bool = false;

    match msg {
        WM_DESTROY => {
            //println!("destroy");
            PostQuitMessage(0);
            //println!("post quit message");
            0
        }
        // Handling the mouse wheel event
        winapi::um::winuser::WM_MOUSEWHEEL => {
            let delta = winapi::um::winuser::GET_WHEEL_DELTA_WPARAM(wparam) as i16;
            add_winevent(WinEvent::MouseWheel(delta));
            0
        }
        winapi::um::winuser::WM_MOUSEMOVE => {
                let x_pos = get_x_lparam(lparam);
                let y_pos = get_y_lparam(lparam);

            if MOUSE_PAN_ACTIVE || MOUSE_MOVE_WITHOUT_SHIFT_ACTIVE {
                let delta_x = x_pos.saturating_sub(LAST_MOUSE_POS.0 as i16);
                let delta_y = y_pos.saturating_sub(LAST_MOUSE_POS.1 as i16);

                if MOUSE_PAN_ACTIVE {
                    add_winevent(WinEvent::MousePan(delta_x as i16, delta_y as i16));
                } else if MOUSE_MOVE_WITHOUT_SHIFT_ACTIVE {
                    add_winevent(WinEvent::MouseRot(delta_x as i16, delta_y as i16));
                }
            }

            LAST_MOUSE_POS = (x_pos as i16, y_pos as i16);
            0
        }
        winapi::um::winuser::WM_MBUTTONDOWN => {
                LAST_MOUSE_POS = (
                    get_x_lparam(lparam),
                    get_y_lparam(lparam),
                );

                if winapi::um::winuser::GetKeyState(winapi::um::winuser::VK_SHIFT) < 0 {
                    MOUSE_PAN_ACTIVE = true;
                } else {
                    MOUSE_MOVE_WITHOUT_SHIFT_ACTIVE = true;
            }
            0
        }
        winapi::um::winuser::WM_MBUTTONUP => {
            MOUSE_PAN_ACTIVE = false;
            MOUSE_MOVE_WITHOUT_SHIFT_ACTIVE = false;
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// define a type to represent the a pointer to the D3D11CreateDeviceAndSwapChain functions
type D3D11CreateDeviceAndSwapChainFN = extern "system" fn (
    pAdapter: *mut IDXGIAdapter,
    DriverType: D3D_DRIVER_TYPE,
    Software: HMODULE,
    Flags: UINT,
    pFeatureLevels: *const D3D_FEATURE_LEVEL,
    FeatureLevels: UINT,
    SDKVersion: UINT,
    pSwapChainDesc: *const DXGI_SWAP_CHAIN_DESC,
    ppSwapChain: *mut *mut IDXGISwapChain,
    ppDevice: *mut *mut ID3D11Device,
    pFeatureLevel: *mut D3D_FEATURE_LEVEL,
    ppImmediateContext: *mut *mut ID3D11DeviceContext,
) -> HRESULT;


/// Enabling this will prevent MM from hooking, see load_d3d11 comment below
const USE_DEBUG_DEVICE:bool = false; 
/// When running with MM, if this is false, it may stack overflow when creating the d3d resources, in the debug build,
/// for unknown reasons.  the release build does not appear to S.O.
/// The SO appears to be related to the d3d load thread MM uses when the device says it is multithread.
#[cfg(debug_assertions)]
const SINGLE_THREADED_DEVICE: bool = true; 
#[cfg(not(debug_assertions))]
const SINGLE_THREADED_DEVICE: bool = false; 
/// Whether to render or not.
const RENDER:bool = true;
/// If > 0, the program will not sometimes render the full geometry causing a "miss" in modelmod,
/// in other words no matching mod which is what it does most of the time in practice.
/// (If this is true and RENDER is true and a valid MOD exists for the prim/vert count, 
/// this will cause the mod to flicker on screen 
/// since nothing is drawn otherwise).  The value is the chance of a miss.
const RENDER_SIMULATE_MISS_FREQ:f32 = 0.0; // 0.0005; 
/// If set, when a miss occurs (see RENDER_SIMULATE_MISS_FREQ) this number of primitives/vertexes 
/// will be used to draw instead of a random amount.  This allows a different mod to be selected for the miss.
/// These values must be <= the primary prim/vert counts used for the program.  The 
/// program will exit with an error if they are greater.
const RENDER_MISS_PRIMVERT_COUNT:Option<(u32,u32)> = None; // Some((7630,5435));

// load the d3d11 library and obtain a pointer to the D3D11CreateDevice function
unsafe fn load_d3d11() -> Option<D3D11CreateDeviceAndSwapChainFN> {
    
    let d3d11 = unsafe { 
        let wide: Vec<u16> = "d3d11.dll\0".encode_utf16().collect();   // note trailing '\0'

        // debug layers only work when we load the real d3d11.dll, but this prevents MM from hooking.
        // tried various ways to fix this 
        // including loading the "d3d11_3SDKLayers.dll" manually, or loading this dll first and then 
        // MM - the problem stems I think from the fact they are named the same so either the real lib
        // or MM gets confused - the thing which came closest to working is renaming mm's hook dll 
        // and loading it after as in the commented out code below - but then MM get's messed up because
        // the managed code has a bunch of imports specifically looking for the name "d3d11.dll".  so i gave 
        // up and the debug layers only work when not using MM - still useful for making sure the device 
        // scaffolding here isn't throwing errors.
        // it may be that the layers lib is looking for specific imports in the d3d11.dll that my hook 
        // variant does not provide, so that is why it fails.  o3 LLM couldn't figure it out either
        // (had me going in circles)
        let d3d11 = if USE_DEBUG_DEVICE {
            eprintln!("==> Warning: Loading d3d11.dll from system path to support debug device - MM will not initialize");
            // load the "real" from the system path
            let d3d11 = LoadLibraryExW(
            wide.as_ptr(), 
            null_mut(), 
                LOAD_LIBRARY_SEARCH_SYSTEM32);
            if d3d11 == null_mut() {
                panic!("failed to load real d3d 11.dll")
            }

            d3d11

            // now copy the "d3d11.dll" in the executable's dir to "mm_d3d11.dll" and load it, then return 
            // a pointer to that as d3d11
            // let d3d11_path = std::env::current_exe()
            //     .expect("Failed to get current exe path")
            //     .parent()
            //     .expect("Failed to get parent directory")
            //     .join("d3d11.dll");

            // let mm_d3d11_path = std::env::current_exe()
            //     .expect("Failed to get current exe path")
            //     .parent()
            //     .expect("Failed to get parent directory")
            //     .join("mm_d3d11.dll");

            // std::fs::copy(&d3d11_path, &mm_d3d11_path)
            //     .expect("Failed to copy d3d11.dll to mm_d3d11.dll");

            // let mm_d3d11 = LoadLibraryW(mm_d3d11_path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>().as_ptr());
            // if mm_d3d11 == null_mut() {
            //     panic!("failed to load mm_d3d11.dll")
            // }
            // mm_d3d11

        } else {
            let d3d11 = LoadLibraryW(wide.as_ptr());
            d3d11
        };
        d3d11
    };    

    if d3d11 == std::ptr::null_mut() {
        println!("failed to load d3d11.dll");
        return None
    }
        
    let d3d11_create_device = unsafe { GetProcAddress(d3d11, b"D3D11CreateDeviceAndSwapChain\0".as_ptr() as *const i8) };
    if d3d11_create_device == std::ptr::null_mut() {
        println!("failed to get D3D11CreateDevice");
        return None
    }
    Some(std::mem::transmute(d3d11_create_device))
}

// Use the specified create device function to create a d3d11 device
fn create_d3d11_device(window:HWND, create_dev_fn: D3D11CreateDeviceAndSwapChainFN)
    -> anyhow::Result<(*mut ID3D11Device,*mut ID3D11DeviceContext, *mut IDXGISwapChain)> {
    let mut device = std::ptr::null_mut();
    let mut context = std::ptr::null_mut();
    let mut swapchain: *mut IDXGISwapChain = std::ptr::null_mut();

    // init the swap chain DXGI_SWAP_CHAIN_DESC description
    let desc:DXGI_SWAP_CHAIN_DESC = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 800, 
            Height: 600,
            RefreshRate: DXGI_RATIONAL {
                Numerator: 60,
                Denominator: 1
            },
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, //DXGI_USAGE_RENDER_TARGET_OUTPUT, // DXGI_USAGE_BACK_BUFFER ?
        BufferCount: 2,
        OutputWindow: window,
        Windowed: 1,
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH
    };

    let mut feature_level = D3D_FEATURE_LEVEL_11_0;
    let dtype = D3D_DRIVER_TYPE_HARDWARE;
    let mut flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT
        | if USE_DEBUG_DEVICE { D3D11_CREATE_DEVICE_DEBUG } else { 0 };

    // if the SINGLE_THREADED_DEVICE constant is true, change the flags to specify a single threaded device
    if SINGLE_THREADED_DEVICE {
        flags |= D3D11_CREATE_DEVICE_SINGLETHREADED;
    }

    let hr = {
        println!("creating device");
        println!("Note: MM will print a message with log file path, but if it successfully initialized the log will be created in the $letter:\\ModelMod\\Logs directory");
        create_dev_fn(
            std::ptr::null_mut(),
            dtype,
            std::ptr::null_mut(),
            flags,
            std::ptr::null_mut(),
            0,
            D3D11_SDK_VERSION,
            &desc,
            &mut swapchain,
            &mut device,
            &mut feature_level,
            &mut context
        )
    };
    if hr != 0 {
        if hr == DXGI_ERROR_SDK_COMPONENT_MISSING {
            eprintln!("device creation failed due to missing sdk component (DXGI_ERROR_SDK_COMPONENT_MISSING)");
        }
        return Err(anyhow!("failed to create d3d11 device: {:X}", hr))

    }
    println!("created d3d11 device: feature level: {:X}", feature_level);

    if USE_DEBUG_DEVICE { 
        // enable more logging which can be seen in debug view when running with this
        unsafe {
            let mut infoq: *mut ID3D11InfoQueue = null_mut();
            let hr_qi = (*device).QueryInterface(
                &IID_ID3D11InfoQueue as REFIID,
                &mut infoq as *mut _ as *mut _,
            );
            if SUCCEEDED(hr_qi) && !infoq.is_null() {
                // 1. make sure output isn’t muted
                (*infoq).SetMuteDebugOutput(FALSE);

                // 2. remove any filters that drop INFO-level messages
                (*infoq).ClearStorageFilter();
                (*infoq).ClearRetrievalFilter();

                // 3. (optional) set breakpoints on more severe messages
                (*infoq).SetBreakOnSeverity(D3D11_MESSAGE_SEVERITY_CORRUPTION, TRUE);
                (*infoq).SetBreakOnSeverity(D3D11_MESSAGE_SEVERITY_ERROR,       TRUE);
                (*infoq).SetBreakOnSeverity(D3D11_MESSAGE_SEVERITY_WARNING,     FALSE);
                (*infoq).SetBreakOnSeverity(D3D11_MESSAGE_SEVERITY_INFO,        FALSE);

                // done with the queue
                (*infoq).Release();
            }

        }
    }

    Ok((device,context,swapchain))
}


/// Call `get_indices` to get the indices and create an index buffer, return the index buffer.
fn create_index_buffer<F>(device: *mut ID3D11Device, nindex: u32, index_fn: F) -> anyhow::Result<*mut ID3D11Buffer>
where F: FnOnce(u32) -> Vec<u16> {
    let indices = index_fn(nindex);
    let index_size = std::mem::size_of::<u16>();
    let index_count = indices.len();
    let index_buffer_size = index_size * index_count;

    let mut index_buffer = std::ptr::null_mut();
    let index_buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: index_buffer_size as u32,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_INDEX_BUFFER,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0
    };
    let index_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: indices.as_ptr() as *const c_void,
        SysMemPitch: 0,
        SysMemSlicePitch: 0
    };

    let hr = unsafe {
        (*device).CreateBuffer(
            &index_buffer_desc,
            &index_data,
            &mut index_buffer
        )
    };
    if hr != 0 {
        return Err(anyhow!("failed to create index buffer: {:X}", hr))
    }
    println!("created index buffer: {:X}", index_buffer as usize);
    Ok(index_buffer)
}

/// Create a vertex buffer from the specified vector of vertices.  Return the buffer and
/// the size of each vertex.
unsafe fn create_vertex_buffer<F>(device: *mut ID3D11Device, vert_fn:F) -> anyhow::Result<(*mut ID3D11Buffer,usize)> 
 //vertices:&[u8], vertex_size: usize
where F: FnOnce() -> (Vec<u8>, usize)
{
    let (vertices,vertex_size) = vert_fn();
    let vertex_buffer_size = vertices.len();

    let mut vertex_buffer = std::ptr::null_mut();
    //let mut vertex_data = std::ptr::null_mut();
    let vertex_buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: vertex_buffer_size as u32,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_VERTEX_BUFFER,
        CPUAccessFlags: 0,
        MiscFlags: 0,
        StructureByteStride: 0
    };
    let vertex_subresource_data = D3D11_SUBRESOURCE_DATA {
        pSysMem: vertices.as_ptr() as *const c_void,
        SysMemPitch: 0,
        SysMemSlicePitch: 0
    };

    let hr = (*device).CreateBuffer(&vertex_buffer_desc,
        &vertex_subresource_data, &mut vertex_buffer);
    if hr != 0 {
        return Err(anyhow!("failed to create vertex buffer: {:X}", hr))
    }
    println!("created vertex buffer: {:X}", vertex_buffer as usize);
    Ok((vertex_buffer,vertex_size))
}

/// create a input layout
unsafe fn create_vertex_layout(device: *mut ID3D11Device, opts: &RunOpts) -> 
    anyhow::Result<(*mut ID3D11InputLayout, Vec<D3D11_INPUT_ELEMENT_DESC>)> {
    let (layout_desc, vshader) = 
        if let (Some(shader_out_file), Some(vert_elems)) = 
            (&opts.vshader_out_file, &opts.vert_elems) {
            println!("using layout args from cli: {} elems; shader out file: {:?}", vert_elems.len(), shader_out_file);
            // Use the vertex elements and shader output file from the options
            let vshader = std::fs::read(shader_out_file)?;
            (vert_elems.clone(), vshader)
        } else {
            panic!("no vertex format was specified");
        };

    let num_elements = layout_desc.len();
    let layout_desc_vec = layout_desc;
    let layout_desc = layout_desc_vec.as_ptr();

    let pVShaderBytecode = vshader.as_ptr() as *const c_void;
    let BytecodeLength = vshader.len();

    println!("read {} bytes of vertex shader bytecode", BytecodeLength);

    let mut ppInputLayout: *mut ID3D11InputLayout = std::ptr::null_mut();
    let res = (*device).CreateInputLayout(
        layout_desc,
        num_elements as u32,
        pVShaderBytecode,
        BytecodeLength,
        &mut ppInputLayout,
    );
    if res != 0 {
        return Err(anyhow!("failed to create input layout: {:X}", res));
}
    println!("created layout");
    Ok((ppInputLayout,layout_desc_vec))
}

pub fn print_input_element_desc(elements: &[D3D11_INPUT_ELEMENT_DESC]) {
    let (_, value_to_name) = shadercomp::read_formats().expect("doh");
    for element in elements {
        let semantic_name = unsafe { 
            std::ffi::CStr::from_ptr(element.SemanticName).to_str().unwrap_or("Invalid UTF-8") };

        println!(
            "Semantic: {}, SemanticIndex: {}, Format: {}, AlignedByteOffset: {}",
            semantic_name,
            element.SemanticIndex,
            value_to_name.get(&element.Format).expect("doh").to_uppercase(),
            element.AlignedByteOffset,
        );
    }
}

pub fn read_elem_file(elemfile: &str) -> Result<Vec<D3D11_INPUT_ELEMENT_DESC>, Box<dyn std::error::Error>> {
    let elements = shadercomp::read_vertex_format(elemfile)?;

    println!("read {} elements from {}", elements.len(), elemfile);
    print_input_element_desc(&elements);

    Ok(elements)
}

enum Mesh {
    BlankPriorToModLoad,
    Shape
}

struct RunOpts {
    pub prim_count:usize,
    pub vert_count:usize,
    pub vshader_out_file:Option<PathBuf>,
    pub pshader_out_file:Option<PathBuf>,
    pub vert_elems: Option<Vec<D3D11_INPUT_ELEMENT_DESC>>,
    pub mesh: Mesh,
    pub tex0path:Option<String>,
}

impl RunOpts {
    pub fn has_custom_vert(&self) -> bool {
        self.vshader_out_file.is_some() && 
        self.vert_elems.as_ref().map(|elems| !elems.is_empty()).unwrap_or(false)
    }
}

// parse any options and the first non-argument option on the command line line which should be a string of the form
// 100,200 representing a prim and vertex count to use, if this argument is not found return an error
fn parse_command_line() -> anyhow::Result<RunOpts> {
    let args: Vec<String> = std::env::args().collect();
    //eprintln!("{:?}", args);


    let mut prim_and_vertex_arg: Option<String> = None;

    let mut i = 1;  // start from 1 to skip the program name
    let mut vshader_out_file:Option<PathBuf> = None;
    let mut pshader_out_file:Option<PathBuf> = None;
    let mut vert_elems: Option<Vec<D3D11_INPUT_ELEMENT_DESC>> = None;

    let mut mesh = Mesh::BlankPriorToModLoad;
    let mut tex0path: Option<String> = None;

    fn comp_shader(path:&str, is_vertex:bool) -> anyhow::Result<PathBuf> {
        match shadercomp::compile_shader(path, is_vertex) {
            Ok(sout) => Ok(sout),
            Err(e) => Err(anyhow!("Failed to compile shader {}: {:?}", path, e)),
        }
    }
    
    while i < args.len() {
        match args[i].as_str() {
            "-shape" => {
                mesh = Mesh::Shape;
                i += 1;
            }
            "-cs" => {
                panic!("the -cs option is now -vs (for vertex shader)");
            }
            "-tex"
            | "-tex0" => {
                if let Some(filename) = args.get(i + 1) {
                    tex0path = Some(filename.clone());
                    i += 1
                }
            }
            "-vs" => {
                if let Some(filename) = args.get(i + 1) {
                    vshader_out_file = Some(comp_shader(filename, true)?);
                    i += 2; // Skip the filename in the next iteration
                } else {
                    println!("Error: No filename provided after '-vs'.");
                    return Err(anyhow!("Error: No filename provided after '-vs'."));
                }
            }
            "-ps" => {
                if let Some(filename) = args.get(i + 1) {
                    pshader_out_file = Some(comp_shader(filename, false)?);
                    i += 2; // Skip the filename in the next iteration
                } else {
                    println!("Error: No filename provided after '-ps'.");
                    return Err(anyhow!("Error: No filename provided after '-ps'."));
                }
            }
            // if there is an -ef argument, read the filename after it and store the output in the "elems" local var
            "-vf"
            | "-ef" => {
                if let Some(filename) = args.get(i + 1) {
                    vert_elems = Some(read_elem_file(filename).expect("failed to read elem file"));
                    
                    i += 2; // Skip the filename in the next iteration
                } else {
                    println!("Error: No filename provided after '-ef'.");
                    return Err(anyhow!("Error: No filename provided after '-ef'."));
                }
            }

            arg if !arg.starts_with('-') && prim_and_vertex_arg.is_none() => {
                prim_and_vertex_arg = Some(arg.to_string());
                i += 1;
            }
            arg if arg.starts_with('-') => {
                panic!("unrecognized argument: {}", arg)
            }
            _ => {
                i += 1;
            }
        }
    }

    let arg_with_counts = prim_and_vertex_arg.ok_or_else(|| {
        anyhow!("Usage: primcount,vertcount")
    })?;
    let mut split = arg_with_counts.split(',');

    let prim_count = split
        .next()
        .ok_or_else(|| anyhow!("failed to parse prim count (expected prim,vert)"))?
        .parse::<usize>()?;

    let vert_count = split
        .next()
        .ok_or_else(|| anyhow!("failed to parse vert count (expected prim,vert)"))?
        .parse::<usize>()?;

    if vert_elems.is_none() {
        vert_elems = Some(read_elem_file("basic_format.txt").expect("failed to read default vert elem file: 'basic_format.txt'"));
    }

    if RENDER {
        if vshader_out_file.is_none() {
            vshader_out_file = Some(comp_shader("shape_vshader.hlsl", true)?)
        }
        if pshader_out_file.is_none() {
            pshader_out_file = Some(comp_shader("shape_pshader.hlsl", false)?)
        }
    }

    let opts = RunOpts {
        prim_count,
        vert_count,
        vshader_out_file,
        pshader_out_file,
        vert_elems,
        mesh,
        tex0path,
    };
    Ok(opts)
}

unsafe fn runapp() -> anyhow::Result<()> {
    let opts = parse_command_line()?;
    let RunOpts { prim_count, vert_count, .. } = opts;

    static SZ_CLASS: &'static [u8] = b"c\0l\0a\0s\0s\0\0\0";
    static SZ_TITLE: &'static [u8] = b"t\0i\0t\0l\0e\0\0\0";
    //static SZ_TEXT: &'static [u8] = b"Window";

    let h_instance = unsafe { GetModuleHandleA(0 as LPCSTR) };
    let wndclass = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_VREDRAW|CS_HREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: h_instance,
        hIcon: 0 as HICON,
        hCursor: 0 as HCURSOR,
        hbrBackground: (COLOR_WINDOWFRAME) as HBRUSH,
        lpszMenuName: 0 as LPCWSTR,
        lpszClassName: SZ_CLASS.as_ptr() as *const u16,
        hIconSm: 0 as HICON,
    };
    unsafe {
        // sometimes this fails??
        let res = RegisterClassExW(&wndclass);
        if res == 0 {
            return Err(anyhow!("failed to register wnd class"))
        }
        let winwidth = 800;
        let winheight = 600;
        let window = CreateWindowExW(
            0,
            SZ_CLASS.as_ptr() as *const u16,
            SZ_TITLE.as_ptr() as *const u16,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT, winwidth, winheight,
            0 as HWND,
            0 as HMENU,
            h_instance,
            0 as LPVOID
        );
        if window == std::ptr::null_mut() {
            let error = GetLastError();
            return Err(anyhow!("failed to create window: {:X}", error))
        }

        ShowWindow(window, SW_SHOWDEFAULT);

        // load d3d11
        let create_dev_fn = load_d3d11().ok_or_else(|| anyhow!("failed to load d3d11"))?;
        println!("create_dev_fn: {:x}", create_dev_fn as usize);
        let (device,context, swapchain)
            = create_d3d11_device(window, create_dev_fn)?;
        println!("device: {:x}", device as usize);
        println!("context: {:x}", context as usize);

        let (layout,layout_vec) = create_vertex_layout(device, &opts)?;
        let vert_size = shadercomp::get_vert_size(&layout_vec).expect("failed to compute vert size");

        if vert_size % 4 != 0 && vert_size % 2 != 0 {
            println!("WARNING: unaligned vertex size found, possible alignment issue; vert size: {}", vert_size);
        }

        let num_indices = prim_count * 3;
        let (vert_data,index_data) = 
            match opts.mesh {
                Mesh::BlankPriorToModLoad => {
                    if !opts.has_custom_vert() {
                        panic!("a vertex format must be specified (-ef))")
                    } else {
                        // use the specified verts format
                        let vec = get_empty_vertices(vert_size, vert_count)
                            .expect("failed to create empty vert buf");
                        (vec, get_indices((num_indices) as u32))
                    }
                },
                Mesh::Shape => {
                    if vert_size != std::mem::size_of::<shape::Vertex>() {
                        panic!("vert size must match shape::Vertex")
                    }
                    let (vert_data,index_data) = shape::generate_cube_mesh();
                    (vert_data, index_data)
                }
            };
        println!("created vert data buf sized {} for {} verts of size {}", vert_data.len(), vert_count, vert_size);

        let num_indices = index_data.len();

        let (vertex_buffer, vert_size) = 
            create_vertex_buffer(device, move || (vert_data, vert_size) )?;
        let index_buffer = create_index_buffer(
            device, num_indices as u32, |_num_indices| index_data)?; //.try_into().expect("can't convert to u32?")?;

        let (miss_prims, vertbuf_miss, indexbuf_miss) = if let Some((miss_prim, miss_vert)) = RENDER_MISS_PRIMVERT_COUNT {
            if miss_prim >= prim_count as u32 || miss_vert >= vert_count as u32 {
                panic!("miss primitives/vertex counts must be lower than program argument prim,verts");
            }
            let vert_data = get_empty_vertices(vert_size, miss_vert as usize)
                .expect("failed to create empty vert buf for miss case");
            let index_data = get_indices((miss_prim * 3) as u32);

            let (vertex_buffer,vsize) = create_vertex_buffer(device, || (vert_data, vert_size))?;
            if vsize != vert_size {
                panic!("oops miss vertex buffer size does not match");
            }
            let index_buffer = create_index_buffer(device, (miss_prim * 3) as u32, |_| index_data)?;

            (miss_prim, vertex_buffer, index_buffer)
        } else {
            (0, null_mut(), null_mut())
        };

        let rend_data = if RENDER {
            Some(render::create_data(device)?)
        } else {
            None
        };

        let vshader = if let Some(ref sfile) = opts.vshader_out_file {
            let vshader = std::fs::read(sfile)?;
            let pVShaderBytecode = vshader.as_ptr() as *const c_void;
            let BytecodeLength = vshader.len();
            let mut pVShader: *mut ID3D11VertexShader = std::ptr::null_mut();
            println!("read {} bytes from {:?}", BytecodeLength, sfile);
            let hr = (*device).CreateVertexShader(pVShaderBytecode, 
                BytecodeLength, 
                null_mut(), 
                &mut pVShader as *mut _);
            if hr == 0 && pVShader != null_mut() {
                Some(pVShader)
            } else {
                eprintln!("error: failed to create vertex shader, device will likely hang after first draw call: {:X}", hr);
                None
            }
        } else {
            None
        };

        if let Some(vshader) = vshader {
            (*context).VSSetShader(vshader, std::ptr::null_mut(), 0);
        }

        // As with the vertex shader, if `opts.pshader_out_file` is set, load and create it on the device and set it on the context
        let _pshader = if let Some(ref sfile) = opts.pshader_out_file {
            let pshader = std::fs::read(sfile)?;
            let pPShaderBytecode = pshader.as_ptr() as *const c_void;
            let BytecodeLength = pshader.len();
            let mut pPShader: *mut ID3D11PixelShader = std::ptr::null_mut();
            println!("read {} bytes from {:?}", BytecodeLength, sfile);
            let hr = (*device).CreatePixelShader(
                pPShaderBytecode,
                BytecodeLength,
                null_mut(),
                &mut pPShader as *mut _,
            );
            if hr == 0 && pPShader != null_mut() {
                (*context).PSSetShader(pPShader, std::ptr::null_mut(), 0);
            } else {
                eprintln!("Error: Failed to create pixel shader: {:X}", hr);
            }
            Some(pPShader)
        } else {
            None
        };

        let has_tex0 = if let Some(filename) = opts.tex0path {
            let mm_root = util::find_mm_root()?;
            let dp = d3dx::deviceptr_from_d3d11(device).ok_or_else(|| anyhow!("failed to get device pointer for d3dx"))?;

            d3dx::load_lib_and_set_in_globalstate(&Some(mm_root), &dp)
                .map_err(|e| anyhow!("d3dx error: {:?}", e))?;
            println!("loaded d3dx (for texture loading)");
            let tex = d3dx::load_texture_strpath(dp, &filename)
                .map_err(|e| anyhow!("d3dx error: {:?}", e))?
                .as_d3d11tex().ok_or_else(|| anyhow!("expected a d3d11 texture to be loaded"))?;
            let tex0srv = d3dx::create_d3d11_srv_from_tex(dp, tex as *mut ID3D11Texture2D)
                .map_err(|e| anyhow!("d3dx error: {:?}", e))?;

            let sresources: [*mut ID3D11ShaderResourceView; 1] = [tex0srv];
            (*context).PSSetShaderResources(0, 1, sresources.as_ptr());

            let ts = render::create_texture_sampler(device)?;
            let samplers: [*mut ID3D11SamplerState; 1] = [ts];
            (*context).PSSetSamplers(0, 1, samplers.as_ptr());
            true
        } else {
            false
        };



        // to actually render something need to create some target buffers
        // 1. Get back buffer from swap chain
        let mut back_buffer: *mut ID3D11Texture2D = std::ptr::null_mut();
        (*swapchain).GetBuffer(
            0,
            &ID3D11Texture2D::uuidof(),
            &mut back_buffer as *mut _ as *mut _,
        );

        // 2. Create Render Target View
        let mut render_target_view: *mut ID3D11RenderTargetView = std::ptr::null_mut();
        (*device).CreateRenderTargetView(
            back_buffer as *mut _,
            std::ptr::null(),
            &mut render_target_view,
        );

        let depth_desc = D3D11_TEXTURE2D_DESC {
            Width: 800,
            Height: 600,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_D24_UNORM_S8_UINT, // 24-bit depth + 8-bit stencil
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_DEPTH_STENCIL,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };

        let mut depth_texture: *mut ID3D11Texture2D = std::ptr::null_mut();
        let hr = {
            (*device).CreateTexture2D(&depth_desc, std::ptr::null(), &mut depth_texture)
        };
        if !SUCCEEDED(hr) {
            panic!("Failed to create depth texture: HRESULT = 0x{:08x}", hr);
        }
        let mut depth_stencil_view: *mut ID3D11DepthStencilView = std::ptr::null_mut();
        let hr = {
            (*device).CreateDepthStencilView(
                depth_texture as *mut _,
                std::ptr::null(), // default view desc
                &mut depth_stencil_view,
            )
        };
        if !SUCCEEDED(hr) {
            panic!("Failed to create depth stencil view: HRESULT = 0x{:08x}", hr);
        }

        // 3. Set RTV on context
        (*context).OMSetRenderTargets(1, &render_target_view, depth_stencil_view);

        use winapi::um::d3d11::D3D11_VIEWPORT;

        let mut swapchain_desc: DXGI_SWAP_CHAIN_DESC = std::mem::zeroed();
        if 0 != (*swapchain).GetDesc(&mut swapchain_desc) {
            panic!("failed to get swapchain description")
        }
        println!("swap buffer size: {}/{}", swapchain_desc.BufferDesc.Width, swapchain_desc.BufferDesc.Height);

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: swapchain_desc.BufferDesc.Width as f32,
            Height: swapchain_desc.BufferDesc.Height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        (*context).RSSetViewports(1, &viewport);

        // the test shape uses this winding, for game meshes we don't really know depends on the game
        let front_cc = if let Mesh::Shape = opts.mesh {
            1
        } else {
            0
        };

        let rasterizer_desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_FRONT, // Disable backface culling
            //CullMode: winapi::um::d3d11::D3D11_CULL_NONE,
            FrontCounterClockwise: front_cc,  
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: 1,        // Enable clipping (typical)
            ScissorEnable: 0,
            MultisampleEnable: 0,
            AntialiasedLineEnable: 0,
        };

        let mut rasterizer_state: *mut ID3D11RasterizerState = std::ptr::null_mut();
        let hr = {
            (*device).CreateRasterizerState(&rasterizer_desc, &mut rasterizer_state)
        };

        if !SUCCEEDED(hr) {
            panic!("Failed to create rasterizer state: HRESULT = 0x{:08x}", hr);
        }

        (*context).RSSetState(rasterizer_state);

        println!("setting vertex buffers with stride size {}", vert_size);
        // set up these slices outside the loop so they don't get dropped 
        let pStrides = [vert_size as u32];
        let pOffsets = [0];
        let ppVertexBuffers = [vertex_buffer];
        let ppVertexBuffers = ppVertexBuffers;
        let missPPVertexBuffers = [vertbuf_miss];
        
        let the_beginning = SystemTime::now();
        let mut msg;
        //let mut start = SystemTime::now();
        let mut start = Instant::now();
        let mut done = false;
        let mut dip_calls: i32 = 0;
        let mut total_dip_calls = 0;
        let mut info_start = SystemTime::now();
        let mut removed_once = false;
        let mut zoom: f32 = 0.0;
        let (mut pan_x, mut pan_y): (i16, i16) = (0,0);
        let (mut rot_x, mut rot_y): (f32, f32) = (0.0,0.0);
        // if true, once per second print out results of last draw call (primitives counts and # of shader invocations)
        let query_draw_results = false; 
        let orbit_cam = true;
        let autoexit_after_secs = 0;

        let target = Duration::from_micros(1000); // 1000 fps cap
        
        
        let mut next_tick = Instant::now();
        

        while !done {
            let mut seclog = false;
            if SystemTime::now().duration_since(info_start).expect("whatever").as_secs() >= 1 {
                println!("dip calls: {}, prim/vert count: {:?}", dip_calls, (prim_count,vert_count));
                total_dip_calls += dip_calls;
                dip_calls = 0;
                info_start = SystemTime::now();
                seclog = true;

                // let mut vp:D3D11_VIEWPORT = std::mem::zeroed();
                // let mut numvp:u32 = 1;
                // (*context).RSGetViewports(&mut numvp as *mut _, &mut vp as *mut _);
                // eprintln!("{} {} {} {} {} {}", vp.Width, vp.Height, vp.TopLeftX, vp.TopLeftY, vp.MinDepth, vp.MaxDepth);
            }
            if autoexit_after_secs > 0 && SystemTime::now().duration_since(the_beginning).expect("oops time issue").as_secs() >= autoexit_after_secs {
                println!("auto-exiting");
                std::process::exit(0);
            }
            let dev_removed_reason = (*device).GetDeviceRemovedReason();
            if !removed_once && dev_removed_reason != 0 {
                total_dip_calls += dip_calls;

                removed_once = true;
                use winapi::shared::winerror::*;

                print!("warning: device removed after {} draw calls, reason: ", total_dip_calls);
                match dev_removed_reason {
                    DXGI_ERROR_DEVICE_HUNG => println!("{}", &format!("device hung")),
                    DXGI_ERROR_DEVICE_REMOVED => println!("{}", &format!("device removed")),
                    DXGI_ERROR_DEVICE_RESET => println!("{}", &format!("device reset")),
                    DXGI_ERROR_DRIVER_INTERNAL_ERROR => println!("{}", &format!("driver internal error")),
                    DXGI_ERROR_INVALID_CALL => println!("{}", &format!("invalid call")),
                    _ => println!("{}", &format!("unknown device removed reason")),
                }
            }

            msg = std::mem::zeroed();
            while PeekMessageW(&mut msg, 0 as HWND, 0, 0, PM_REMOVE) != 0 {
                //println!("pm msg {:x}", msg.message);
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
                if msg.message == WM_CLOSE
                    || msg.message == WM_QUIT
                    || msg.message == WM_DESTROY {
                    done = true;
                }
            }
            if done {
                break;
            }


            
            {
                match WINEVENTS.lock() {
                    Ok(mut evts) => {
                        for e in evts.iter() {
                            match e {
                                WinEvent::MouseWheel(delta) => {
                                    if !orbit_cam {
                                        zoom = (zoom as i32 + (*delta as i32 * 1000)) as f32;

                                        // clamp zoom to range (-render::ZOOM_MAX, render::ZOOM_MAX)
                                        zoom = zoom.clamp(-render::ZOOM_MAX as f32, render::ZOOM_MAX as f32);
                                    } else {
                                        const ZOOM_SENS : f32 = 0.005;
                                        zoom = zoom + (*delta as f32 * ZOOM_SENS);
                                    }
                                    
                                    println!("zoom {}", zoom);
                                },
                                WinEvent::MousePan(x, y) => {
                                    pan_x = pan_x.saturating_add( (- *x * 50) as i16 );
                                    pan_y = pan_y.saturating_add( (- *y * 50) as i16 );
                                    
                                    //println!("pan: {pan_x},{pan_y}")
                                },
                                WinEvent::MouseRot(x, y) => {
                                    const ROT_SENS: f32 = 0.005;
                                    rot_x += *x as f32 * ROT_SENS;
                                    rot_y += *y as f32 * ROT_SENS;
                                }
                            }
                        }
                        evts.clear();
                    },
                    Err(e) => eprintln!("failed to lock winevents: {:?}", e)
                }
            }

            if Instant::now() < next_tick {
                std::thread::sleep(next_tick - Instant::now());
            }
            next_tick += target;

            let now = Instant::now();
            

            // if the sync interval in present below is > than zero this will slow down the dip rate to 
            // the something like the refresh rate of display or some safe value like 30/sec
            let mut do_present = true;
            // "render" some stuff
            {
                {
                    let color = [0.0, 0.2, 0.2, 1.0];

                    (*context).ClearRenderTargetView(render_target_view, &color);
                    (*context).ClearDepthStencilView(
                        depth_stencil_view,
                        D3D11_CLEAR_DEPTH | D3D11_CLEAR_STENCIL,
                        1.0,
                        0,
                    );
                }

                // if not rendering we don't set any constants, but we need to call once to trigger MM's hook code
                let buffer = std::ptr::null_mut();
                let buffers = [buffer];
                (*context).VSSetConstantBuffers(0, 1, buffers.as_ptr());

                if let Some(ref rend_data) = rend_data {
                    let elapsed_sec = SystemTime::now().duration_since(the_beginning).expect("time went backwards?").as_secs_f32();
                    let time_angle = elapsed_sec as f32 * std::f32::consts::FRAC_PI_2;

                    let eye = Vec3::new(0.0, 0.0, -5.0);

                    //println!("{}", time_angle);
                    let rx = 0.0; //time_angle / 2.0; //time_angle;// 
                    let ry = time_angle / 4.0;

                    let (rotation,origin) = if let Mesh::Shape = opts.mesh {
                        let origin = Vec3::ZERO;
                        let rz = time_angle;
                        let rotation = Vec3::new(rx, ry, rz);
                        (rotation,origin)
                    } else {
                        let origin = Vec3::new(0.0, 2.0, 0.0);
                        let rz = 0.0;
                        let rotation = Vec3::new(rx, ry, rz);
                        (rotation,origin)
                    };
                    
                    let aspect_ratio = 
                        swapchain_desc.BufferDesc.Width as f32
                        / swapchain_desc.BufferDesc.Height as f32;
                    let fov_y_radians = std::f32::consts::FRAC_PI_4; // 45 degrees
                    let z_near = 0.1;
                    let z_far = 100.0;
                    let light_dir = Vec3::new(0.0, -1.0, -1.0); // pointing diagonally down-forward

                    let frustum = render::FrustumParams {
                        aspect_ratio,
                        fov_y_radians,
                        z_near,
                        z_far
                    };

                    let mvp_p = if !orbit_cam {
                        ModelViewParams::FixedCam { 
                            zoom: zoom as i32, 
                            pan: (pan_x,pan_y), 
                            origin: origin, 
                            eye: eye, 
                            rotation_radians: rotation, 
                            frustum,
                        }
                    } else {
                        let mut radius = 5.0;
                        radius = (radius - (zoom * 0.5)).max(0.1);

                        ModelViewParams::OrbitCam { 
                            orbit_angles: Vec2::new(rot_x,rot_y), 
                            radius: radius, 
                            pan: (pan_x,pan_y), 
                            pivot: Vec3 { x: 0.0, y: 0.0, z: 0.0 }, 
                            model_rotation: rotation, 
                            frustum 
                        }
                    };

                    prepare_shader_constants(
                        context,
                        rend_data,
                        &mvp_p,
                        light_dir,
                        has_tex0,
                    )?;
                } 

                //println!("setting index buffer");
                (*context).IASetIndexBuffer(index_buffer, DXGI_FORMAT_R16_UINT, 0);

                (*context).IASetVertexBuffers(0, 1,
                    ppVertexBuffers.as_ptr(), pStrides.as_ptr(), pOffsets.as_ptr());

                // sanity check since this has bitten me in the ass before 
                // (from using insta-dropped vertex buffer arrays due to a trailing as_ptr()
                // on the let binding on the slices passed to IASetVertexBuffers above, 
                // causing garbage pointers get passed and garbage stride to get set)
                {
                    let mut got_vb: *mut ID3D11Buffer = std::ptr::null_mut();
                    let mut got_stride: u32 = 0;
                    let mut got_offset: u32 = 0;
                    {
                        (*context).IAGetVertexBuffers(0, 1, &mut got_vb, &mut got_stride, &mut got_offset);
                        assert_eq!(got_stride, vert_size as u32, "stride {} != vert size {}", got_stride, vert_count);
                        assert_eq!(got_stride % 4, 0, "want stride % 4 but got {}", got_stride); // this has to be aligned
                        
                        if !got_vb.is_null() { (*got_vb).Release(); }
                    }
                }
                //println!("setting index layout");
                (*context).IASetInputLayout(layout);
                //println!("set topology");
                (*context).IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                (*context).RSSetState(rasterizer_state);

                // {
                //     (*context).OMSetDepthStencilState(std::ptr::null_mut(), 0);      // kill depth tests
                //     let blend = [0.0f32; 4];
                //     (*context).OMSetBlendState(std::ptr::null_mut(), &blend, 0xFFFF_FFFF);
                // }

                let max_prims = num_indices / 3;

                if RENDER {
                    do_present = true;
                }

                let primCount = if let Mesh::Shape = opts.mesh {
                    max_prims
                } else if RENDER_SIMULATE_MISS_FREQ > 0.0 {
                    // a small pct of the time, draw the target primCount, the rest draw a random number of
                    // prims up to primCount (simulates the high miss rate in mod rendering)
                    if rand::random::<f32>() > RENDER_SIMULATE_MISS_FREQ {
                        max_prims
                    } else {
                        if miss_prims > 0 {
                            (*context).IASetIndexBuffer(indexbuf_miss, DXGI_FORMAT_R16_UINT, 0);

                            (*context).IASetVertexBuffers(0, 1,
                                missPPVertexBuffers.as_ptr(), pStrides.as_ptr(), pOffsets.as_ptr());
                            miss_prims as usize
                        } else {
                            rand::random::<usize>() % max_prims
                        }
                    }
                } else {
                    max_prims
                };
                let IndexCount = primCount * 3;

                let q = if query_draw_results && seclog {
                    let qd = D3D11_QUERY_DESC { Query: D3D11_QUERY_PIPELINE_STATISTICS, MiscFlags: 0 };
                    let mut q: *mut ID3D11Query = std::ptr::null_mut();
                    if 0 != (*device).CreateQuery(&qd, &mut q) {
                        panic!("failed to create query")
                    }
                    (*context).Begin(q as *mut ID3D11Asynchronous);
                    q
                } else {
                    std::ptr::null_mut()
                };

                (*context).DrawIndexed(IndexCount as u32, 0, 0);
                dip_calls += 1;

                if query_draw_results && !q.is_null() {
                    (*context).End(q as *mut ID3D11Asynchronous);
                    let mut stats: D3D11_QUERY_DATA_PIPELINE_STATISTICS = std::mem::zeroed();
                    while (*context).GetData(q as _, &mut stats as *mut _ as *mut _, std::mem::size_of_val(&stats) as u32, 0) == winapi::shared::winerror::S_FALSE {}
                    println!(
                        "IA prims: {}  VS invoc: {}  PS invoc: {}",
                        stats.IAPrimitives, stats.VSInvocations, stats.PSInvocations
                    );
                    (*q).Release();
                }

            }
            // "present"
            if do_present {
                start = now;

                (*swapchain).Present(0, 0);
            }
        }
    };
    Ok(())
}

fn main() {
    // use env to figure out mode, default is run d3d app
    let mode = std::env::var("MODE").unwrap_or("d3d".to_string());
    if mode == "mmobj" {
        let res = test_load_mmobj();
        println!("res: {:?}", res);
        return;
    }
    unsafe {
        let res = runapp();
        println!("res: {:?}", res);
    };
}
