extern crate winapi;

use std::ffi::{CString, c_void};
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::os::raw::{c_char, c_uint};
use winapi::shared::minwindef::{FARPROC, HMODULE};
use winapi::um::libloaderapi::{LoadLibraryA, GetProcAddress};

// HRESULT and type aliases
type HRESULT = i32;
type LPCVOID = *const c_void;
type SIZE_T = usize;
type UINT = c_uint;
type LPCSTR = *const c_char;
type LPVOID = *mut c_void;

// Minimal IUnknown for ID3DBlob
#[repr(C)]
struct ID3DBlobVtbl {
    pub QueryInterface: extern "system" fn(*mut ID3DBlob, *const c_void, *mut *mut c_void) -> HRESULT,
    pub AddRef: extern "system" fn(*mut ID3DBlob) -> u32,
    pub Release: extern "system" fn(*mut ID3DBlob) -> u32,
    pub GetBufferPointer: extern "system" fn(*mut ID3DBlob) -> LPVOID,
    pub GetBufferSize: extern "system" fn(*mut ID3DBlob) -> SIZE_T,
}
#[repr(C)]
pub struct ID3DBlob {
    pub lpVtbl: *const ID3DBlobVtbl,
}
impl ID3DBlob {
    unsafe fn get_buffer_pointer(&self) -> *const u8 {
        ((*self.lpVtbl).GetBufferPointer)(self as *const _ as *mut _) as *const u8
    }
    unsafe fn get_buffer_size(&self) -> usize {
        ((*self.lpVtbl).GetBufferSize)(self as *const _ as *mut _)
    }
    unsafe fn release(&self) {
        ((*self.lpVtbl).Release)(self as *const _ as *mut _);
    }
}

// D3DCompile type signature
type D3DCompileFn = unsafe extern "system" fn(
    pSrcData: LPCVOID,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const c_void,
    pInclude: *const c_void,
    pEntryPoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    ppCode: *mut *mut ID3DBlob,
    ppErrorMsgs: *mut *mut ID3DBlob,
) -> HRESULT;

pub fn compile_shader(sourcefile:&str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Load DLL
    let dll_path = CString::new("d3dcompiler_47.dll")?;
    let hmod: HMODULE = unsafe { LoadLibraryA(dll_path.as_ptr()) };
    if hmod.is_null() {
        return Err("Failed to load d3dcompiler_47.dll".into());
    }

    // Get D3DCompile symbol
    let func_name = CString::new("D3DCompile")?;
    let proc: FARPROC = unsafe { GetProcAddress(hmod, func_name.as_ptr()) };
    if proc.is_null() {
        return Err("Failed to get D3DCompile proc address".into());
    }
    let d3d_compile: D3DCompileFn = unsafe { std::mem::transmute(proc) };

    // Read shader file
    let shader_code = fs::read_to_string(sourcefile)?;

    // Prepare arguments
    let entry = CString::new("main")?;
    let target = CString::new("vs_5_0")?;
    let mut blob: *mut ID3DBlob = ptr::null_mut();
    let mut error_blob: *mut ID3DBlob = ptr::null_mut();

    let hr = unsafe {
        d3d_compile(
            shader_code.as_ptr() as LPCVOID,
            shader_code.len(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            entry.as_ptr(),
            target.as_ptr(),
            0, // flags
            0,
            &mut blob,
            &mut error_blob,
        )
    };

    if hr < 0 {
        if !error_blob.is_null() {
            unsafe {
                let msg_ptr = (*error_blob).get_buffer_pointer();
                let msg_len = (*error_blob).get_buffer_size();
                let msg = std::slice::from_raw_parts(msg_ptr, msg_len);
                eprintln!("D3DCompile error: {}", String::from_utf8_lossy(msg));
                (*error_blob).release();
            }
        } else {
            eprintln!("D3DCompile failed with HRESULT 0x{:08X}", hr as u32);
        }
        return Err("Compilation failed".into());
    }

    // Use the shader bytecode (for example, write it to file)
    unsafe {
        let byte_ptr = (*blob).get_buffer_pointer();
        let byte_len = (*blob).get_buffer_size();
        let bytes = std::slice::from_raw_parts(byte_ptr, byte_len);
        use std::path::{Path, PathBuf};

        // Determine the output file path
        let source_path = Path::new(sourcefile);
        let mut output_path = PathBuf::from(source_path);
        output_path.set_extension("cso");

        // Write the shader bytecode to the output file
        fs::write(&output_path, bytes)?;
        println!("Shader {} compiled successfully ({} bytes)", sourcefile, byte_len);
        (*blob).release();

        Ok(output_path)
    }
    
}

use std::collections::HashMap;
use std::io::{self, BufRead};

pub fn read_formats() -> Result<(HashMap<String, u32>, HashMap<u32, String>), Box<dyn std::error::Error>> {
    // Initialize hash maps
    let mut name_to_value = HashMap::new();
    let mut value_to_name = HashMap::new();

    // Open the file
    let file = fs::File::open("dxgiformats.txt")?;
    let reader = io::BufReader::new(file);

    // Process each line
    for line in reader.lines() {
        let line = line?;
        // Find the position of the comment and trim the line for either '//' or '#'
        let trimmed = {
                if let Some(pos) = line.find("//").or_else(|| line.find('#')) {
                &line[..pos]
            } else {
                &line
            }
        }.trim();
        // Skip if the line is empty after trimming
        if trimmed.is_empty() {
            continue;
        }

        // Split each line into name and value
        if let Some((name, value_str)) = trimmed.split_once('=') {
            let name = name.trim().to_lowercase();
            let value: u32 = value_str.trim().parse()?;

            // Insert into hash maps
            name_to_value.insert(name.clone(), value);
            value_to_name.insert(value, name);
        }
    }

    Ok((name_to_value, value_to_name))
}

use winapi::shared::dxgiformat::DXGI_FORMAT;
use winapi::um::d3d11::{D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA};

pub fn read_vertex_format(filename: &str) -> Result<Vec<D3D11_INPUT_ELEMENT_DESC>, Box<dyn std::error::Error>> {
    let (name_to_value, _) = read_formats()?;
    let supported_semantics = vec![
        "position", "color", "normal", "binormal",
        "texcoord", "blendweight", "blendindices",
    ];

    let file = fs::File::open(filename)?;
    let reader = io::BufReader::new(file);

    let mut input_descriptions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        // Find the position of the comment and trim the line for either '//' or '#'
        let trimmed_line = {
                if let Some(pos) = line.find("//").or_else(|| line.find('#')) {
                &line[..pos]
            } else {
                &line
            }
        }.trim();
        // Skip if the line is empty after trimming
        if trimmed_line.is_empty() {
            continue;
        }
        

        let parts: Vec<&str> = trimmed_line.split_whitespace().collect();
        if parts.len() != 4 {
            return Err(format!("Invalid format in line: {}", line).into());
        }

        let semantic_name = parts[0].to_lowercase();
        if !supported_semantics.contains(&semantic_name.as_str()) {
            return Err(format!("Unsupported semantic: {}", semantic_name).into());
        }

        let semantic_index: u32 = parts[1].parse()?;
        let aligned_byte_offset: u32 = parts[2].parse()?;
        let format_name = parts[3].to_lowercase();

        // Attempt to get the DXGI format directly or by prefixing the format name
        let dxgi_format = match name_to_value.get(&format_name)
            .or_else(|| {
                let prefixed_format_name = format!("dxgi_format_{}", format_name);
                name_to_value.get(&prefixed_format_name)
            }) {
            Some(&format) => format,
            None => return Err(format!("Unsupported format: {}", format_name).into()),
        };

        // Create a null-terminated CStr for the semantic name
        let cstr_semantic_name = CString::new(semantic_name.to_uppercase())?;

        // Leak the CString to get a static pointer, so it remains valid for the lifetime of the descriptor
        let cstr_semantic_name_ptr = Box::leak(Box::new(cstr_semantic_name)).as_ptr();

        // Create the input element descriptor
        let input_element_desc = D3D11_INPUT_ELEMENT_DESC {
            SemanticName: cstr_semantic_name_ptr,
            SemanticIndex: semantic_index,
            Format: dxgi_format as DXGI_FORMAT,
            InputSlot: 0,                      // Default slot
            AlignedByteOffset: aligned_byte_offset,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,           // Default step rate
        };

        input_descriptions.push(input_element_desc);
    }

    Ok(input_descriptions)
}

pub fn get_vert_size(elems: &Vec<D3D11_INPUT_ELEMENT_DESC>) -> Result<usize, Box<dyn std::error::Error>> {
    use winapi::shared::dxgiformat::*;
    // Define hardcoded sizes for common DXGI formats
    let format_sizes = HashMap::from([
        (DXGI_FORMAT_B8G8R8A8_UNORM, 4),
        (DXGI_FORMAT_R16G16B16A16_SINT, 8),
        (DXGI_FORMAT_R16G16_SINT, 4),
        (DXGI_FORMAT_R16G16B16A16_SNORM, 8),
        (DXGI_FORMAT_R16G16_SNORM, 4),
        (DXGI_FORMAT_R8G8B8A8_UINT, 4),
        (DXGI_FORMAT_R32_FLOAT, 4),
        (DXGI_FORMAT_R32G32_FLOAT, 8),
        (DXGI_FORMAT_R32G32B32_FLOAT, 12),
        (DXGI_FORMAT_R32G32B32A32_FLOAT, 16),
        (DXGI_FORMAT_R8G8B8A8_UNORM, 4),
    ]);

    if elems.is_empty() {
        return Err("No elements found in input descriptions".into());
    }

    let mut max_offset = None;
    let mut max_format_size = 0;

    for elem in elems {
        let end_offset = elem.AlignedByteOffset as usize;
        if max_offset.is_none() || end_offset > max_offset.unwrap_or(0) {
            max_offset = Some(end_offset);
            // Check the size of the format and update the maximum offset
            let format_size = match format_sizes.get(&elem.Format) {
                Some(&size) => size,
                None => return Err(format!("Unsupported or unknown format: {}", elem.Format).into()),
            };
            
            max_format_size = format_size;
        }
    }

    // The vertex size is determined by the maximum offset reached by any element plus its format size
    match max_offset {
        Some(offset) => Ok(offset + max_format_size),
        None => Err("Failed to determine maximum offset".into()),
    }
}
