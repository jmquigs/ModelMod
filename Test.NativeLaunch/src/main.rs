#![allow(non_snake_case)]

/*
This standalone program allows testing of the entire code base without injecting into a
real game exe.  This program acts like a d3d11 game, making appropriate calls to "render" geometry
although it doesn't actually render anything.  However, MM is hooked into the process
(via the `runmm.sh` script), so it can intercept and perform actions on these rendering calls
like it normally would.

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

A fair portion of this code was written by github copilot.  So, it's not very good.  But it's
good enough to test the mod loading process.  It's also a good example of how to use the
d3d11 api, which is not well documented. I've tried to add comments to explain what's going on.
In this paragraph everything was written by copilot except for the first sentence and this one.

Vertex Layout notes:
`simple_vertex_shader.dat` was snapped binary-only from a game.  It is required to create input
layouts (the api validates the shader bytecode against the input layout description).

If you want to use other vertex formats you'll need to snap shaders for those as well,
and modify this code to create structs for the that format.  And define a new structure
like SimpleVertex.  In modelmod you can add some code to `hook_CreateInputLayoutFn`
and dump out shaders there, along with the pointer values of the input layout that is created.
Later when the mod is rendered, you can dump out the pointer value in the log so that you know
which shader is used by the mod.

You can also just write and compile a vertex shader that matches the format you want.  I tried
to do that with `fxc` but it didn't like my shader and I didn't bother trying to figure it out.

When creating a new format, pay careful attention to the vertex size and the byte alignment
values and format types in the input layout description.  These should match what the
game is reporting for the mod you are interested in.  You may need to pad your vertex structure
to meet the alignment requirements.  See `SimpleVertex` and `get_simple_layout_description`
below for examples.

Note, layout description arrays cannot be simply captured from the game like the shaders can,
because they contain a raw ascii pointer to the semantic name, which will crash here if you
tro to use it.

*/

use std::time::SystemTime;

use anyhow::Ok;
use winapi::{um::{winuser::{CreateWindowExW, WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, ShowWindow,
    WM_QUIT, TranslateMessage, DispatchMessageW,
    PeekMessageW, PM_REMOVE, WNDCLASSEXW, CS_VREDRAW, CS_HREDRAW,
    PostQuitMessage, DefWindowProcW, COLOR_WINDOWFRAME, RegisterClassExW, SW_SHOWDEFAULT, WM_CLOSE, WM_DESTROY},
    libloaderapi::{GetModuleHandleA, LoadLibraryA, GetProcAddress},
    d3d11::{ID3D11Device, ID3D11DeviceContext, D3D11_SDK_VERSION,
        D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA, ID3D11InputLayout,
        ID3D11Buffer, D3D11_BUFFER_DESC, D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER,
        D3D11_SUBRESOURCE_DATA, D3D11_BIND_INDEX_BUFFER},
        d3dcommon::{D3D_DRIVER_TYPE_HARDWARE, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST}},
    shared::{windef::{HWND, HMENU, HICON, HCURSOR, HBRUSH},
    minwindef::{LPVOID, UINT, WPARAM, LPARAM, LRESULT, HMODULE},
    ntdef::{LPCSTR, LPCWSTR, HRESULT},
    dxgi::{IDXGIAdapter, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD,
        IDXGISwapChain, DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH},
        dxgitype::{DXGI_MODE_DESC, DXGI_RATIONAL, DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            DXGI_MODE_SCALING_UNSPECIFIED, DXGI_SAMPLE_DESC, DXGI_USAGE_RENDER_TARGET_OUTPUT},
        dxgiformat::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R32G32B32_FLOAT,
            DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R8G8B8A8_UINT,
            DXGI_FORMAT_R16_UINT}}, ctypes::c_void};

use winapi::um::d3dcommon::{D3D_DRIVER_TYPE,D3D_FEATURE_LEVEL};

use winapi::um::errhandlingapi::GetLastError;

use crate::load_mmobj::test_load_mmobj;

#[macro_use]
extern crate anyhow;

mod load_mmobj;
mod interop_mmobj;

#[repr(C, align(8))]
struct SimpleVertex {
    position: [f32; 3],
    blend_indices: [u8; 4],
    blend_weights: [u8; 4],
    //normal: [f32; 3],
    unused: [u8; 12], // due to align byte offset for texcoord of 32
    texcoord: [f32; 2],
    //tangent: [f32; 3],
    //binormal: [f32; 3],
}

fn get_simple_layout_description() -> Vec<D3D11_INPUT_ELEMENT_DESC> {
    vec![
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"POSITION\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"BLENDWEIGHT\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            InputSlot: 0,
            AlignedByteOffset: 12,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"BLENDINDICES\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R8G8B8A8_UINT,
            InputSlot: 0,
            AlignedByteOffset: 16,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0
        },
        // D3D11_INPUT_ELEMENT_DESC {
        //     SemanticName: b"NORMAL\0".as_ptr() as *const i8,
        //     SemanticIndex: 0,
        //     Format: DXGI_FORMAT_R32G32B32_FLOAT,
        //     InputSlot: 0,
        //     AlignedByteOffset: 12,
        //     InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        //     InstanceDataStepRate: 0
        // },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: b"TEXCOORD\0".as_ptr() as *const i8,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 32,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0
        },
        // D3D11_INPUT_ELEMENT_DESC {
        //     SemanticName: b"TANGENT\0".as_ptr() as *const i8,
        //     SemanticIndex: 0,
        //     Format: DXGI_FORMAT_R32G32B32_FLOAT,
        //     InputSlot: 0,
        //     AlignedByteOffset: 32,
        //     InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        //     InstanceDataStepRate: 0
        // },
        // D3D11_INPUT_ELEMENT_DESC {
        //     SemanticName: b"BINORMAL\0".as_ptr() as *const i8,
        //     SemanticIndex: 0,
        //     Format: DXGI_FORMAT_R32G32B32_FLOAT,
        //     InputSlot: 0,
        //     AlignedByteOffset: 44,
        //     InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        //     InstanceDataStepRate: 0
        // },
        // D3D11_INPUT_ELEMENT_DESC {
        //     SemanticName: b"BLENDWEIGHT\0".as_ptr() as *const i8,
        //     SemanticIndex: 0,
        //     Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        //     InputSlot: 0,
        //     AlignedByteOffset: 56,
        //     InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
        //     InstanceDataStepRate: 0
        // },
    ]
}


unsafe extern "system"
fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // println!("wnd proc: {:x}", msg);

    match msg {
        WM_DESTROY => {
            //println!("destroy");
            PostQuitMessage(0);
            //println!("post quit message");
            0
        },
        _ => {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
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

// load the d3d11 library and obtain a pointer to the D3D11CreateDevice function
unsafe fn load_d3d11() -> Option<D3D11CreateDeviceAndSwapChainFN> {
    let d3d11 = unsafe { LoadLibraryA(b"d3d11.dll\0".as_ptr() as *const i8) };
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
    let mut feature_level = 0;
    let dtype = D3D_DRIVER_TYPE_HARDWARE;

    // init the swap chain DXGI_SWAP_CHAIN_DESC description
    let desc:DXGI_SWAP_CHAIN_DESC = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 640, // ought to be good enough for anybody!
            Height: 480,
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

    let hr = {
        println!("creating device");
        println!("Note: MM will print a message with log file path, but if it successfully initialized the log will be created in the $letter:\\ModelMod\\Logs directory");
        create_dev_fn(
            std::ptr::null_mut(),
            dtype,
            std::ptr::null_mut(),
            0,
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
        return Err(anyhow!("failed to create d3d11 device: {:X}", hr))
    }
    println!("created d3d11 device: feature level: {:X}", feature_level);
    Ok((device,context,swapchain))
}

// generate a vector of the specified number of simple vertices, using random positions, blend indices and weights.
// Constrain the positions to be within the range -100 to 100.
fn get_random_vertices(num_vertices: usize) -> Vec<SimpleVertex> {
    let mut vertices = Vec::new();
    for _ in 0..num_vertices {
        vertices.push(SimpleVertex {
            position: [
                rand::random::<f32>() * 200.0 - 100.0,
                rand::random::<f32>() * 200.0 - 100.0,
                rand::random::<f32>() * 200.0 - 100.0,
            ],
            blend_indices: [
                rand::random::<u8>(),
                rand::random::<u8>(),
                rand::random::<u8>(),
                rand::random::<u8>(),
            ],
            blend_weights: [
                rand::random::<u8>(),
                rand::random::<u8>(),
                rand::random::<u8>(),
                rand::random::<u8>(),
            ],
            texcoord: [0.0, 0.0],
            unused: [0;12],
            //normal: [0.0, 0.0, 0.0],
            //tangent: [0.0, 0.0, 0.0],
            //binormal: [0.0, 0.0, 0.0],
        });
    }
    vertices
}

// generate a index buffer of up to N indicies, using random indicies.
fn get_indices(n:u32) -> Vec<u16> {
    let mut indices = Vec::new();
    for _ in 0..n {
        indices.push(rand::random::<u16>());
    }
    indices
}

// Call `get_indices` to get the indices and create an index buffer, return the index buffer.
fn create_index_buffer(device: *mut ID3D11Device, nindex:u32) -> anyhow::Result<*mut ID3D11Buffer> {
    let indices = get_indices(nindex);
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

// Create a vertex buffer from the specified vector of vertices.  Return the buffer and
// the size of each vertex.
unsafe fn create_vertex_buffer(device: *mut ID3D11Device, vertices:&Vec<SimpleVertex>) -> anyhow::Result<(*mut ID3D11Buffer,usize)> {
    let vertex_size = std::mem::size_of::<SimpleVertex>();
    let vertex_count = vertices.len();
    let vertex_buffer_size = vertex_size * vertex_count;

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

// create a input layout
unsafe fn create_vertex_layout(device: *mut ID3D11Device) -> anyhow::Result<*mut ID3D11InputLayout> {
    // create the input layout using semantics position, normal, texcoord, tangent, binormal, blendweight,
    // and blendindices
    let layout_desc = get_simple_layout_description();

    let num_elements = layout_desc.len();
    let layout_desc = layout_desc.as_ptr();

    // load the shader bytecode from the file "simple_vertex_shader.dat"
    let vshader = std::fs::read("simple_vertex_shader.dat")?;
    let pVShaderBytecode = vshader.as_ptr() as *const c_void;
    let BytecodeLength = vshader.len() as usize;
    println!("read {} bytes of vertex shader bytecode", BytecodeLength);

    let mut ppInputLayout: *mut ID3D11InputLayout = std::ptr::null_mut();
    let res = (*device).CreateInputLayout(layout_desc,
        num_elements as u32, pVShaderBytecode,
        BytecodeLength, &mut ppInputLayout);
    if res != 0 {
        return Err(anyhow!("failed to create input layout: {:X}", res))
    }
    println!("created layout");
    Ok(ppInputLayout)

}

// parse the first argument of the command line which should be a string of the form
// 100,200 representing a prim and vertex count to use, if this argument is not found return
// an error
fn parse_command_line() -> anyhow::Result<(usize, usize)> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(anyhow!("usage: {} <prim count>,<vertex count>", args[0]))
    }
    let mut split = args[1].split(',');
    let prim_count = split.next()
        .ok_or_else(|| anyhow!("failed to parse prim count (expected prim,vert)"))?
        .parse::<usize>()?;
    let vertex_count = split.next()
        .ok_or_else(|| anyhow!("failed to parse vert count (expected prim,vert)"))?
        .parse::<usize>()?;
    Ok((prim_count, vertex_count))
}

unsafe fn runapp() -> anyhow::Result<()> {
    let (primCount, vertCount) = parse_command_line()?;

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

        let layout = create_vertex_layout(device)?;

        let verts = get_random_vertices(vertCount);

        let (vertex_buffer, vert_size) = create_vertex_buffer(device, &verts)?;
        let index_buffer = create_index_buffer(
            device, (primCount * 3).try_into().expect("can't conert to u32?"))?;

        let mut msg;
        let mut start = SystemTime::now();
        let mut done = false;
        let mut dip_calls = 0;
        let mut info_start = SystemTime::now();
        while !done {
            if SystemTime::now().duration_since(info_start).expect("whatever").as_secs() >= 1 {
                println!("dip calls: {}, prim/vert count: {:?}", dip_calls, (primCount,vertCount));
                dip_calls = 0;
                info_start = SystemTime::now();
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
            let now = SystemTime::now();
            let _elapsed = now.duration_since(start).expect("whatever").as_millis();
            let mut do_present = false;
            // "render" some stuff
            {
                // call VSSetConstantBuffers to set the constant buffer on the context, this will
                // trigger some MM rehook code
                let buffer = std::ptr::null_mut();
                (*context).VSSetConstantBuffers(0, 1, &buffer);

                //println!("setting index buffer");
                (*context).IASetIndexBuffer(index_buffer, DXGI_FORMAT_R16_UINT, 0);

                //println!("setting vertex buffers");
                let pStrides = [vert_size as u32].as_ptr();
                let pOffsets = [0].as_ptr();
                let ppVertexBuffers = [vertex_buffer];
                let ppVertexBuffers = ppVertexBuffers.as_ptr();

                (*context).IASetVertexBuffers(0, 1,
                    ppVertexBuffers, pStrides, pOffsets);
                //println!("setting index layout");
                (*context).IASetInputLayout(layout);
                //println!("set topology");
                (*context).IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

                // a small pct of the time, draw the target primCount, the rest draw a random number of
                // prims up to primCount (simulates the high miss rate in mod rendering)
                let primCount = if rand::random::<f32>() < 0.05 {
                    primCount
                } else {
                    rand::random::<usize>() % primCount
                };

                let IndexCount = primCount * 3;

                (*context).DrawIndexed(IndexCount as u32, 0, 0);
                dip_calls += 1;

                if dip_calls > 20000 {
                    // slow down cowboy, no need to burn cpu, 20K is enough to trigger the hook's
                    // "periodic" processing heuristics
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    do_present = true;
                }

            }
            // "present"
            if do_present {
                start = now;

                // clear the buffer using the d3d context
                //let color = [1.0, 0.0, 0.0, 1.0];

                //(*context).ClearRenderTargetView(std::ptr::null_mut(), &color);

                // swap the buffers.  we don't care about this since we don't render anything
                // but the device probably works more realisticaly if there is a present after X
                // amount of drawing.
                (*swapchain).Present(1, 0);
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
