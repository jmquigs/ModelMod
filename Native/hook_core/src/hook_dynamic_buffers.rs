//! Experimental capture of dynamically-updated DX11 index/vertex buffers for snapshotting.
//!
//! Compiled only with the `snapshot-dynamic-buffers` feature.  Some games pack many meshes into a
//! large, dynamically-updated vertex/index buffer (a "megabuffer") that is created empty and filled
//! later via `Map`/`Unmap` (often `WRITE_NO_OVERWRITE`) or `UpdateSubresource` -- neither of which
//! `hook_CreateBuffer` sees.  These hooks copy the buffer bytes whenever the game fills them so the
//! snapshot code can read them back.
//!
//! This is disabled by default because copying these (often large) buffers every frame significantly
//! slows the game; it is only useful for occasional manual snapshotting of such meshes.

use std::time::SystemTime;

use winapi::ctypes::c_void;
use winapi::shared::minwindef::UINT;
use winapi::shared::winerror::E_FAIL;
use winapi::um::winnt::HRESULT;
use winapi::um::d3d11::{ID3D11DeviceContext, ID3D11Resource,
    D3D11_MAP, D3D11_MAPPED_SUBRESOURCE, D3D11_BOX,
    D3D11_MAP_WRITE, D3D11_MAP_WRITE_DISCARD, D3D11_MAP_WRITE_NO_OVERWRITE, D3D11_MAP_READ_WRITE};

use global_state::GLOBAL_STATE;
use device_state::{dev_state_d3d11_read, dev_state_d3d11_write};
use shared_dx::dx11rs::DX11RenderState;
use shared_dx::util::write_log_file;

use crate::hook_render_d3d11::get_hook_context;

/// True for map types that (may) write the buffer, i.e. those whose contents we want to capture.
#[inline]
fn is_write_map(map_type: D3D11_MAP) -> bool {
    map_type == D3D11_MAP_WRITE
        || map_type == D3D11_MAP_WRITE_DISCARD
        || map_type == D3D11_MAP_WRITE_NO_OVERWRITE
        || map_type == D3D11_MAP_READ_WRITE
}

/// Insert/overwrite captured buffer bytes for a tracked VB/IB.
///
/// A `createtime` entry is pushed only when the key is new.  Pushing a duplicate `(ptr, time)`
/// tuple on every update would let the expiry GC (see `expire_data`) remove still-live data when
/// the oldest tuple's cutoff is reached.  With this rule, a continuously-updated buffer self-heals:
/// after the GC eventually expires it, the next update finds the key absent and re-inserts the
/// data plus a fresh `createtime`, so any subsequent draw/snapshot still sees current bytes.
fn capture_buffer_data(rs: &mut DX11RenderState, is_ib: bool, buf_ptr: usize, data: Vec<u8>) {
    if is_ib {
        let is_new = !rs.device_index_buffer_data.contains_key(&buf_ptr);
        rs.device_index_buffer_data.insert(buf_ptr, data);
        if is_new {
            rs.device_index_buffer_createtime.push((buf_ptr, SystemTime::now()));
        }
    } else {
        let is_new = !rs.device_vertex_buffer_data.contains_key(&buf_ptr);
        rs.device_vertex_buffer_data.insert(buf_ptr, data);
        if is_new {
            rs.device_vertex_buffer_createtime.push((buf_ptr, SystemTime::now()));
        }
    }
}

/// Patch a sub-range of a tracked VB/IB's captured bytes (used for boxed UpdateSubresource).
/// Ensures a full-size (`byte_width`) zero-filled copy exists first, then overwrites
/// `[offset, offset+src.len())`.
fn patch_captured_buffer(rs: &mut DX11RenderState, is_ib: bool, buf_ptr: usize,
    byte_width: usize, offset: usize, src: &[u8]) {
    let (map, ctlist) = if is_ib {
        (&mut rs.device_index_buffer_data, &mut rs.device_index_buffer_createtime)
    } else {
        (&mut rs.device_vertex_buffer_data, &mut rs.device_vertex_buffer_createtime)
    };
    let is_new = !map.contains_key(&buf_ptr);
    let entry = map.entry(buf_ptr).or_insert_with(|| vec![0u8; byte_width]);
    if entry.len() < byte_width {
        entry.resize(byte_width, 0u8);
    }
    let end = offset + src.len();
    if end <= entry.len() {
        entry[offset..end].copy_from_slice(src);
    }
    if is_new {
        ctlist.push((buf_ptr, SystemTime::now()));
    }
}

/// Hooked `ID3D11DeviceContext::Map`.  When precopy is enabled, remembers the CPU pointer of a
/// write-mapped tracked VB/IB so `hook_Unmap` can copy its contents.  Buffers the game fills via
/// Map (e.g. dynamic ring "megabuffers") are otherwise invisible to `hook_CreateBuffer`.
pub unsafe extern "system" fn hook_Map(
    THIS: *mut ID3D11DeviceContext,
    pResource: *mut ID3D11Resource,
    Subresource: UINT,
    MapType: D3D11_MAP,
    MapFlags: UINT,
    pMappedResource: *mut D3D11_MAPPED_SUBRESOURCE,
) -> HRESULT {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return E_FAIL,
    };
    let hr = (hook_context.real_map)(THIS, pResource, Subresource, MapType, MapFlags, pMappedResource);

    if GLOBAL_STATE.run_conf.precopy_data
        && hr == 0
        && Subresource == 0
        && !pMappedResource.is_null()
        && is_write_map(MapType) {
        let cpu_ptr = (*pMappedResource).pData as usize;
        if cpu_ptr != 0 {
            let res_key = pResource as usize;
            // read-lock to check if this is a tracked VB/IB (skips the write lock for the very
            // common constant-buffer Map case), then write-lock only to record the pending map.
            let meta = dev_state_d3d11_read()
                .and_then(|(_lck, state)| state.rs.device_buffer_meta.get(&res_key).copied());
            if let Some((is_ib, byte_width)) = meta {
                dev_state_d3d11_write().map(|(_lock, ds)| {
                    ds.rs.mapped_buffers.insert(res_key, (cpu_ptr, is_ib, byte_width));
                });
            }
        }
    }
    hr
}

/// Hooked `ID3D11DeviceContext::Unmap`.  Copies a pending write-mapped VB/IB's bytes into the
/// snapshot buffer store *before* calling the real Unmap (which invalidates the mapped pointer).
pub unsafe extern "system" fn hook_Unmap(
    THIS: *mut ID3D11DeviceContext,
    pResource: *mut ID3D11Resource,
    Subresource: UINT,
) {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if GLOBAL_STATE.run_conf.precopy_data && Subresource == 0 {
        let res_key = pResource as usize;
        // cheap read-lock membership check; only take the write lock for actually-tracked unmaps.
        let is_pending = dev_state_d3d11_read()
            .map(|(_lck, state)| state.rs.mapped_buffers.contains_key(&res_key))
            .unwrap_or(false);
        if is_pending {
            let pending = dev_state_d3d11_write()
                .and_then(|(_lock, ds)| ds.rs.mapped_buffers.remove(&res_key));
            if let Some((cpu_ptr, is_ib, byte_width)) = pending {
                if cpu_ptr != 0 && byte_width > 0 {
                    // copy outside the lock to minimize lock hold time; cpu_ptr stays valid until
                    // the real Unmap below.
                    let vlen = byte_width as usize;
                    let mut dest_v: Vec<u8> = Vec::with_capacity(vlen);
                    std::ptr::copy_nonoverlapping::<u8>(cpu_ptr as *const u8, dest_v.as_mut_ptr(), vlen);
                    dest_v.set_len(vlen);
                    dev_state_d3d11_write().map(|(_lock, ds)| {
                        capture_buffer_data(&mut ds.rs, is_ib, res_key, dest_v);
                    });
                }
            }
        }
    }

    (hook_context.real_unmap)(THIS, pResource, Subresource);
}

/// Hooked `ID3D11DeviceContext::UpdateSubresource`.  Captures bytes written to a tracked VB/IB
/// for buffers the game updates this way instead of via Map.
pub unsafe extern "system" fn hook_UpdateSubresource(
    THIS: *mut ID3D11DeviceContext,
    pDstResource: *mut ID3D11Resource,
    DstSubresource: UINT,
    pDstBox: *const D3D11_BOX,
    pSrcData: *const c_void,
    SrcRowPitch: UINT,
    SrcDepthPitch: UINT,
) {
    let hook_context = match get_hook_context() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    if GLOBAL_STATE.run_conf.precopy_data && DstSubresource == 0 && !pSrcData.is_null() {
        let res_key = pDstResource as usize;
        let meta = dev_state_d3d11_read()
            .and_then(|(_lck, state)| state.rs.device_buffer_meta.get(&res_key).copied());
        if let Some((is_ib, byte_width)) = meta {
            if byte_width > 0 {
                if pDstBox.is_null() {
                    // full-resource update.
                    let vlen = byte_width as usize;
                    let mut dest_v: Vec<u8> = Vec::with_capacity(vlen);
                    std::ptr::copy_nonoverlapping::<u8>(pSrcData as *const u8, dest_v.as_mut_ptr(), vlen);
                    dest_v.set_len(vlen);
                    dev_state_d3d11_write().map(|(_lock, ds)| {
                        capture_buffer_data(&mut ds.rs, is_ib, res_key, dest_v);
                    });
                } else {
                    // boxed (partial) update: for a buffer, left/right are byte offsets.
                    let left = (*pDstBox).left as usize;
                    let right = (*pDstBox).right as usize;
                    let bw = byte_width as usize;
                    if right > left && right <= bw {
                        let span = right - left;
                        let mut src_copy: Vec<u8> = Vec::with_capacity(span);
                        std::ptr::copy_nonoverlapping::<u8>(pSrcData as *const u8, src_copy.as_mut_ptr(), span);
                        src_copy.set_len(span);
                        dev_state_d3d11_write().map(|(_lock, ds)| {
                            patch_captured_buffer(&mut ds.rs, is_ib, res_key, bw, left, &src_copy);
                        });
                    } else {
                        write_log_file(&format!(
                            "hook_UpdateSubresource: ignoring out-of-range box update (left {}, right {}, byte_width {})",
                            left, right, bw));
                    }
                }
            }
        }
    }

    (hook_context.real_update_subresource)(THIS, pDstResource, DstSubresource, pDstBox, pSrcData, SrcRowPitch, SrcDepthPitch);
}
