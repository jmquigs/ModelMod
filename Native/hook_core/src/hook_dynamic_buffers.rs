//! Experimental capture of DX11 "dynamic buffers" for snapshotting.
//!
//! Compiled only with the `snapshot-dynamic-buffers` feature, and the origin of the "dynamic
//! buffer" shorthand used throughout: an index or vertex buffer whose contents we get by watching
//! the game write it, rather than from the `pInitialData` it was created with.  Some games pack
//! many meshes into one large such buffer (a "megabuffer") that is created empty and filled later
//! via `Map`/`Unmap` (often `WRITE_NO_OVERWRITE`) or `UpdateSubresource` -- neither of which
//! `hook_CreateBuffer` sees.  These hooks copy the buffer bytes whenever the game fills them so the
//! snapshot code can read them back.
//!
//! Capture only runs while a snapshot is actually in progress; see `precopy_capture_active`.

use std::sync::atomic::Ordering;
use std::time::SystemTime;

use winapi::ctypes::c_void;
use winapi::shared::minwindef::UINT;
use winapi::shared::winerror::E_FAIL;
use winapi::um::winnt::HRESULT;
use winapi::um::d3d11::{ID3D11Buffer, ID3D11DeviceContext, ID3D11Resource,
    D3D11_BUFFER_DESC, D3D11_MAP, D3D11_MAPPED_SUBRESOURCE, D3D11_BOX,
    D3D11_BIND_INDEX_BUFFER, D3D11_BIND_VERTEX_BUFFER,
    D3D11_RESOURCE_DIMENSION, D3D11_RESOURCE_DIMENSION_BUFFER,
    D3D11_MAP_WRITE, D3D11_MAP_WRITE_DISCARD, D3D11_MAP_WRITE_NO_OVERWRITE, D3D11_MAP_READ_WRITE};

use global_state::GLOBAL_STATE;
use device_state::{dev_state_d3d11_read, dev_state_d3d11_write};
use shared_dx::dx11rs::{BufferCaptureInfo, BufferMeta, BufferWriteKind, DX11RenderState,
    BUFFER_CAPTURE_SEQ};
use shared_dx::types::DX11Metrics;
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

/// True when the Map/Unmap/UpdateSubresource hooks should actually copy buffer contents.
///
/// Precopy being enabled is necessary but, by default, not sufficient.  A game that packs meshes
/// into a large dynamic buffer typically refills it many times per frame, and copying
/// the whole thing on each of those writes costs orders of magnitude more than the snapshot
/// itself: at tens of MB a pop, a few hundred updates per frame is tens of GB of memcpy per frame.
/// The snapshot only needs the bytes that were written during the frames it is capturing, so by
/// default capture is restricted to the snapshot window (`is_snapping`, which lasts `snap_ms`
/// after the snap key).  Set the `SnapPreCopyAlways` registry dword to 1 for the old always-on
/// behavior, needed only if a game updates its mesh buffers less often than that window.
#[inline]
fn precopy_capture_active() -> bool {
    unsafe {
        GLOBAL_STATE.run_conf.precopy_data
            && (!GLOBAL_STATE.run_conf.precopy_only_when_snapping || GLOBAL_STATE.is_snapping)
    }
}

/// Record the cost of a capture so `process_metrics` can report it.  Called with the device state
/// already write-locked for the copy, so it doesn't cost a second acquisition.
fn note_capture_metrics(metrics: &mut DX11Metrics, bytes: usize, nanos: u64) {
    metrics.dyn_precopy_captures += 1;
    metrics.dyn_precopy_bytes += bytes as u64;
    metrics.dyn_precopy_nanos += nanos;
    if bytes as u32 > metrics.dyn_precopy_largest {
        metrics.dyn_precopy_largest = bytes as u32;
    }
}

/// Record how a buffer's stored bytes were just produced.  Diagnostic only; see
/// `BufferCaptureInfo`.  Called with the state already write-locked for the copy.
fn note_capture_provenance(rs: &mut DX11RenderState, buf_ptr: usize, kind: BufferWriteKind,
    bytes: usize) {
    let seq = BUFFER_CAPTURE_SEQ.fetch_add(1, Ordering::Relaxed);
    let count = rs.buffer_capture_info.get(&buf_ptr).map(|i| i.count).unwrap_or(0) + 1;
    rs.buffer_capture_info.insert(buf_ptr, BufferCaptureInfo { kind, seq, count, bytes });
}

/// Copy `len` bytes from `src` into the stored copy of a tracked VB/IB.
///
/// The destination allocation is reused across updates rather than replaced with a fresh `Vec`.
/// These buffers are large and the game may refill one many times per frame, so allocating and
/// freeing a multi-megabyte block each time costs far more than the copy itself: the allocator
/// hands large blocks straight back to the OS, so every update would fault in a fresh set of
/// zero pages.  Reusing the allocation means holding the write lock across the copy, which is the
/// cheaper of the two.
///
/// A `createtime` entry is pushed only when the key is new.  Pushing a duplicate `(ptr, time)`
/// tuple on every update would let the expiry GC (see `expire_data`) remove still-live data when
/// the oldest tuple's cutoff is reached.  With this rule, a continuously-updated buffer self-heals:
/// after the GC eventually expires it, the next update finds the key absent and re-inserts the
/// data plus a fresh `createtime`, so any subsequent draw/snapshot still sees current bytes.
unsafe fn capture_buffer_data(rs: &mut DX11RenderState, is_ib: bool, buf_ptr: usize,
    src: *const u8, len: usize, kind: BufferWriteKind) {
    note_capture_provenance(rs, buf_ptr, kind, len);
    let (map, ctlist) = if is_ib {
        (&mut rs.device_index_buffer_data, &mut rs.device_index_buffer_createtime)
    } else {
        (&mut rs.device_vertex_buffer_data, &mut rs.device_vertex_buffer_createtime)
    };
    let is_new = !map.contains_key(&buf_ptr);
    let dest = map.entry(buf_ptr).or_insert_with(|| Vec::with_capacity(len));
    dest.clear();
    dest.reserve(len);
    std::ptr::copy_nonoverlapping::<u8>(src, dest.as_mut_ptr(), len);
    dest.set_len(len);
    if is_new {
        ctlist.push((buf_ptr, SystemTime::now()));
    }
}

/// Patch a sub-range of a tracked VB/IB's captured bytes (used for boxed UpdateSubresource).
/// Ensures a full-size (`byte_width`) zero-filled copy exists first, then overwrites
/// `[offset, offset+src.len())`.
unsafe fn patch_captured_buffer(rs: &mut DX11RenderState, is_ib: bool, buf_ptr: usize,
    byte_width: usize, offset: usize, src: *const u8, src_len: usize) {
    note_capture_provenance(rs, buf_ptr, BufferWriteKind::UpdateSubresourceBox(
        offset as u32, (offset + src_len) as u32), byte_width);
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
    let end = offset + src_len;
    if end <= entry.len() {
        std::ptr::copy_nonoverlapping::<u8>(src, entry.as_mut_ptr().add(offset), src_len);
    }
    if is_new {
        ctlist.push((buf_ptr, SystemTime::now()));
    }
}

/// How many buffers the lazy discovery path (`resolve_buffer_meta`) has found so far.  Only used
/// to throttle logging, which is why Relaxed ordering is fine.
static LAZY_MESH_BUFFERS_FOUND: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// Log at most this many lazily discovered mesh buffers, so that enabling precopy in game gives
/// visible confirmation in the log without spamming it for the rest of the session.
const LAZY_MESH_BUFFER_LOG_LIMIT: usize = 20;

/// Ask a resource what it is.  Returns `BufferMeta::Other` for anything that isn't a non-empty
/// vertex or index buffer.
///
/// `GetType` is checked before casting to `ID3D11Buffer`: `ID3D11Buffer` derives from
/// `ID3D11Resource` so the cast is only valid once we know the resource really is a buffer.
unsafe fn query_buffer_meta(res: *mut ID3D11Resource) -> BufferMeta {
    if res.is_null() {
        return BufferMeta::Other;
    }
    let mut dim: D3D11_RESOURCE_DIMENSION = 0;
    (*res).GetType(&mut dim);
    if dim != D3D11_RESOURCE_DIMENSION_BUFFER {
        return BufferMeta::Other;
    }
    let buf = res as *mut ID3D11Buffer;
    let mut desc: D3D11_BUFFER_DESC = std::mem::zeroed();
    (*buf).GetDesc(&mut desc);
    let is_ib = desc.BindFlags & D3D11_BIND_INDEX_BUFFER != 0;
    let is_vb = desc.BindFlags & D3D11_BIND_VERTEX_BUFFER != 0;
    if (is_ib || is_vb) && desc.ByteWidth > 0 {
        BufferMeta::Mesh { is_index: is_ib, byte_width: desc.ByteWidth }
    } else {
        BufferMeta::Other
    }
}

/// Cheap "is this a VB/IB we care about" filter for the dynamic buffer write hot path.
///
/// `hook_CreateBuffer` fills in `device_buffer_meta` at creation time, but it is only hooked when
/// precopy was already enabled when the device was hooked.  When precopy is instead enabled at
/// runtime (`cmd_clear_texture_lists`), every buffer the game already created is missing from the
/// map -- and those are exactly the long-lived "megabuffers" we want, which are typically created
/// once at startup and only ever refilled, so waiting for a `CreateBuffer` that never comes means
/// the runtime toggle can never capture anything.
///
/// So on a miss, ask the resource itself and cache the answer, negative results included: the
/// overwhelming majority of maps are constant buffers, and those must not pay for a `GetType`/
/// `GetDesc` pair on every frame.  The cache is bounded by the number of distinct resources the
/// game creates, so this is effectively a one-time cost per resource.
unsafe fn resolve_buffer_meta(res: *mut ID3D11Resource) -> BufferMeta {
    let res_key = res as usize;
    let cached = dev_state_d3d11_read()
        .and_then(|(_lck, state)| state.rs.device_buffer_meta.get(&res_key).copied());
    if let Some(meta) = cached {
        return meta;
    }

    let meta = query_buffer_meta(res);
    dev_state_d3d11_write().map(|(_lock, ds)| {
        ds.rs.device_buffer_meta.insert(res_key, meta);
    });
    if let BufferMeta::Mesh { is_index, byte_width } = meta {
        let found = LAZY_MESH_BUFFERS_FOUND.fetch_add(1, Ordering::Relaxed);
        if found < LAZY_MESH_BUFFER_LOG_LIMIT {
            write_log_file(&format!(
                "dyn precopy: now tracking {} buffer {:x} ({} bytes) discovered via update hook",
                if is_index { "index" } else { "vertex" }, res_key, byte_width));
            if found == LAZY_MESH_BUFFER_LOG_LIMIT - 1 {
                write_log_file("dyn precopy: (further buffer discoveries will not be logged)");
            }
        }
    }
    meta
}

/// Drop every captured buffer whose stored bytes came from a dynamic write.
///
/// Called from the clear-texture-lists key, which is the point at which the user signals a fresh
/// start (typically after entering a new scene).  Two things go stale across that boundary and
/// neither is self-correcting:
///
/// The maps here are keyed on raw buffer pointers.  Nothing hooks buffer `Release`, so when the
/// game destroys its megabuffers on a scene change and D3D hands the same addresses back for the
/// replacements, the old entries are still sitting there.  A snapshot then reads a dead buffer's
/// bytes for a live buffer and produces a plausible-looking mesh made of the wrong triangles.
///
/// Even without address reuse, a buffer the game filled long ago and has not written through any
/// path we observe since will hand back whatever it last held.
///
/// After this, a dynamic buffer must be captured again before it can be snapshotted, so a
/// snapshot that would previously have produced garbage fails with "was not previously saved"
/// instead.  That is the intended trade.
///
/// Buffers whose bytes came from `pInitialData` are kept: DX11 will not read a buffer back, so
/// dropping those would permanently break snapshotting the ordinary static meshes, which have
/// nothing to do with this problem and are not stale.
pub fn reset_captured_dynamic_buffers() {
    dev_state_d3d11_write().map(|(_lock, ds)| {
        let rs = &mut ds.rs;
        let mut dropped = 0usize;
        let mut freed = 0usize;
        {
            // disjoint field borrows: the provenance map is read while the data maps are pruned.
            let info = &rs.buffer_capture_info;
            // anything we can't positively identify as static initial data is treated as dynamic;
            // every capture path records provenance, so this should not happen in practice.
            let keep = |ptr: &usize| matches!(info.get(ptr),
                Some(i) if i.kind == BufferWriteKind::InitialData);

            rs.device_index_buffer_data.retain(|ptr, data| {
                let k = keep(ptr);
                if !k { dropped += 1; freed += data.len(); }
                k
            });
            rs.device_index_buffer_createtime.retain(|(ptr, _)| keep(ptr));
            rs.device_vertex_buffer_data.retain(|ptr, data| {
                let k = keep(ptr);
                if !k { dropped += 1; freed += data.len(); }
                k
            });
            rs.device_vertex_buffer_createtime.retain(|(ptr, _)| keep(ptr));
        }
        rs.buffer_capture_info.retain(|_, i| i.kind == BufferWriteKind::InitialData);
        // pure caches that re-populate on demand, so clearing them costs nothing and drops
        // whatever was left behind by buffers that have since been released.
        rs.device_buffer_meta.clear();
        rs.mapped_buffers.clear();

        write_log_file(&format!(
            "dyn precopy: dropped {} dynamic buffer(s) ({} bytes); each must be captured \
             again before it can be snapshotted",
            dropped, freed));
    });
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

    if precopy_capture_active()
        && hr == 0
        && Subresource == 0
        && !pMappedResource.is_null()
        && is_write_map(MapType) {
        let cpu_ptr = (*pMappedResource).pData as usize;
        if cpu_ptr != 0 {
            let res_key = pResource as usize;
            // check whether this is a VB/IB we track (a cached lookup for all but the first map
            // of a given resource, which keeps the very common constant-buffer case cheap), then
            // write-lock only to record the pending map.
            if let BufferMeta::Mesh { is_index, byte_width } = resolve_buffer_meta(pResource) {
                dev_state_d3d11_write().map(|(_lock, ds)| {
                    ds.rs.mapped_buffers.insert(res_key, (cpu_ptr, is_index, byte_width, MapType));
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
            if let Some((cpu_ptr, is_ib, byte_width, map_type)) = pending {
                // Re-query rather than trusting the cached metadata before reading byte_width
                // bytes out of the mapped pointer.  The cache is keyed on a raw pointer and D3D
                // reuses freed addresses, so a stale entry could otherwise send us reading past
                // the end of a smaller buffer.  This costs a GetDesc only on unmaps of buffers we
                // actually intend to copy, which is negligible next to the copy itself.
                let current = query_buffer_meta(pResource);
                let confirmed = current == (BufferMeta::Mesh { is_index: is_ib, byte_width });
                if !confirmed {
                    dev_state_d3d11_write().map(|(_lock, ds)| {
                        ds.rs.device_buffer_meta.insert(res_key, current);
                    });
                    write_log_file(&format!(
                        "hook_Unmap: skipping stale capture for resource {:x} (expected {:?}, found {:?})",
                        res_key, BufferMeta::Mesh { is_index: is_ib, byte_width }, current));
                } else if cpu_ptr != 0 && byte_width > 0 {
                    // cpu_ptr stays valid until the real Unmap below.
                    let vlen = byte_width as usize;
                    dev_state_d3d11_write().map(|(_lock, ds)| {
                        let start = SystemTime::now();
                        capture_buffer_data(&mut ds.rs, is_ib, res_key, cpu_ptr as *const u8, vlen,
                            BufferWriteKind::Map(map_type));
                        let nanos = start.elapsed().map(|d| d.as_nanos() as u64).unwrap_or(0);
                        note_capture_metrics(&mut ds.metrics, vlen, nanos);
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

    if precopy_capture_active() && DstSubresource == 0 && !pSrcData.is_null() {
        let res_key = pDstResource as usize;
        // as in hook_Map, this resolves (and caches) the resource type on first sight so that
        // buffers created before a runtime precopy enable are still picked up.
        if let BufferMeta::Mesh { .. } = resolve_buffer_meta(pDstResource) {
            // and as in hook_Unmap, take the size from a fresh query rather than the pointer-keyed
            // cache before reading that many bytes out of pSrcData.
            if let BufferMeta::Mesh { is_index: is_ib, byte_width } = query_buffer_meta(pDstResource) {
                if pDstBox.is_null() {
                    // full-resource update.
                    let vlen = byte_width as usize;
                    dev_state_d3d11_write().map(|(_lock, ds)| {
                        let start = SystemTime::now();
                        capture_buffer_data(&mut ds.rs, is_ib, res_key, pSrcData as *const u8, vlen,
                            BufferWriteKind::UpdateSubresource);
                        let nanos = start.elapsed().map(|d| d.as_nanos() as u64).unwrap_or(0);
                        note_capture_metrics(&mut ds.metrics, vlen, nanos);
                    });
                } else {
                    // boxed (partial) update: for a buffer, left/right are byte offsets.
                    let left = (*pDstBox).left as usize;
                    let right = (*pDstBox).right as usize;
                    let bw = byte_width as usize;
                    if right > left && right <= bw {
                        let span = right - left;
                        dev_state_d3d11_write().map(|(_lock, ds)| {
                            let start = SystemTime::now();
                            patch_captured_buffer(&mut ds.rs, is_ib, res_key, bw, left,
                                pSrcData as *const u8, span);
                            let nanos = start.elapsed().map(|d| d.as_nanos() as u64).unwrap_or(0);
                            note_capture_metrics(&mut ds.metrics, span, nanos);
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
