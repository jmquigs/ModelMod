use shared_dx::error::*;
use shared_dx::types::DevicePointer;
use shared_dx::util;
use global_state::GLOBAL_STATE;

use shared_dx::util::ReleaseOnDrop;

use shared_dx::defs_dx9::*;
use std::ptr::null_mut;
use types::d3dx::*;

//enum ShaderType { Vertex, Pixel }

pub fn is_enabled() -> bool {
    true
}

fn check_hr(hr:i32, context:&str) -> Result<()> {
    if hr != 0 {
        return Err(HookError::CaptureFailed(format!("{}: failed HR: {:X}", context, hr)));
    }
    Ok(())
}

// Old skool "generic" because I'm too lazy to make a Trait to abstract the two variants
macro_rules! impl_save_shader {
    ($name:ident, $ptrtype:ident, $getfn:ident) => {
        unsafe fn $name(device_ptr:*mut IDirect3DDevice9, snap_dir:&str, snap_prefix:&str, suffix:&str) -> Result<bool> {
            let mut shader: *mut $ptrtype = null_mut();
            check_hr( (*device_ptr).$getfn(&mut shader), "get shader")?;
            if shader == null_mut() {
                return Err(HookError::CaptureFailed("no shader".to_owned()));
            }
            let _rod = ReleaseOnDrop::new(shader);
            let mut size:UINT = 0;
            check_hr( (*shader).GetFunction(null_mut(), &mut size), "get shader function size")?;
            if size == 0 {
                return Err(HookError::CaptureFailed("zero size shader".to_owned()));
            }
            if size > 1 * 1024 * 1024 {
                return Err(HookError::CaptureFailed(format!("?? huge shader: {}", size)));
            }
            let mut out_buf: Vec<u8> = vec![0; size as usize];
            let out_ptr = out_buf.as_mut_ptr() as *mut winapi::ctypes::c_void;
            check_hr( (*shader).GetFunction(out_ptr, &mut size), "get shader function data")?;

            let fout = snap_dir.to_owned()  + "/" + snap_prefix + suffix + ".dat";
            use std::io::Write;
            let mut file = std::fs::File::create(&fout)?;
            file.write_all(&out_buf)?;
            util::write_log_file(&format!("wrote {} shader bytes to {}", out_buf.len(), fout));

            // disassemble
            let d3dx_fn = GLOBAL_STATE
                .d3dx_fn
                .as_ref()
                .ok_or(HookError::SnapshotFailed("d3dx not found".to_owned()))?;

            match d3dx_fn {
                D3DXFn::DX9(d3dx_fn) => {
                    let mut buf: *mut ID3DXBuffer = null_mut();
                    let out_ptr = out_ptr as *const DWORD;
                    check_hr( (d3dx_fn.D3DXDisassembleShader)(out_ptr, FALSE, null_mut(), &mut buf), "disassemble")?;
                    let _rod = ReleaseOnDrop::new(buf);
                    let bptr = (*buf).GetBufferPointer() as *mut u8;
                    let bsize = ((*buf).GetBufferSize() - 1) as usize; // last byte is null/garbage, whatev
                    let wslice = std::slice::from_raw_parts(bptr, bsize);
                    let fout = snap_dir.to_owned()  + "/" + snap_prefix + suffix + ".asm";
                    let mut file = std::fs::File::create(&fout)?;
                    file.write_all(wslice)?;
                    util::write_log_file(&format!("wrote shader disassembly to {}", fout));
                },
                _ => {
                    util::write_log_file(&format!("Warning: this d3dx cannot disassemble shaders"));
                },
            }

            Ok(true)
        }
    };
}

impl_save_shader!(save_pixel_shader_d3d9, IDirect3DPixelShader9, GetPixelShader);
impl_save_shader!(save_vertex_shader_d3d9, IDirect3DVertexShader9, GetVertexShader);

pub fn take_snapshot(device:&mut DevicePointer, snap_dir:&str, snap_prefix:&str) -> (bool,bool) {
    unsafe {
        match device {
            DevicePointer::D3D9(device) => {
                let gotpix = save_pixel_shader_d3d9(*device, snap_dir, snap_prefix, "_pshader").unwrap_or_else(|e| {
                    util::write_log_file(&format!("failed to save shader: {:?}", e));
                    false
                });
                let gotvert = save_vertex_shader_d3d9(*device, snap_dir, snap_prefix, "_vshader").unwrap_or_else(|e| {
                    util::write_log_file(&format!("failed to save shader: {:?}", e));
                    false
                });

                (gotpix, gotvert)
            },
            DevicePointer::D3D11(_device) => {
                util::write_log_file("shader capture not implemented for D3D11");
                (false,false)
            },
        }
    }
}