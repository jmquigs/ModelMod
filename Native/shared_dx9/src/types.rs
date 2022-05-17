use crate::defs::*;

pub struct HookDirect3D9 {
    pub real_create_device: CreateDeviceFn,
}

#[derive(Clone)]
pub struct HookDirect3D9Device {
    pub real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
    //pub real_begin_scene: BeginSceneFn,
    pub real_present: PresentFn,
    pub real_release: IUnknownReleaseFn,
    pub real_set_texture: SetTextureFn,
    pub ref_count: ULONG,
    // shader constants
    pub real_set_vertex_sc_f: SetVertexShaderConstantFFn,
    pub real_set_vertex_sc_i: SetVertexShaderConstantIFn,
    pub real_set_vertex_sc_b: SetVertexShaderConstantBFn,
    pub real_set_pixel_sc_f: SetPixelShaderConstantFFn,
    pub real_set_pixel_sc_i: SetPixelShaderConstantIFn,
    pub real_set_pixel_sc_b: SetPixelShaderConstantBFn,
}

pub struct HookD3D9State {
    pub d3d9: Option<HookDirect3D9>,
    pub device: Option<HookDirect3D9Device>,
}

pub struct HookD3D11State {
    pub context: HookDirect3D911Context,
}

pub enum HookDeviceState {
    D3D9(HookD3D9State),
    D3D11(HookD3D11State),
}

pub struct DeviceState {
    pub hook: Option<HookDeviceState>,
    pub d3d_window: HWND,
    pub d3d_resource_count: u32, // TODO: this should be tracked per device pointer.
}

impl HookDirect3D9Device {
    pub fn new(
        real_draw_indexed_primitive: DrawIndexedPrimitiveFn,
        //real_begin_scene: BeginSceneFn,
        real_present: PresentFn,
        real_release: IUnknownReleaseFn,
        real_set_texture: SetTextureFn,
        real_set_vertex_sc_f: SetVertexShaderConstantFFn,
        real_set_vertex_sc_i: SetVertexShaderConstantIFn,
        real_set_vertex_sc_b: SetVertexShaderConstantBFn,
        real_set_pixel_sc_f: SetPixelShaderConstantFFn,
        real_set_pixel_sc_i: SetPixelShaderConstantIFn,
        real_set_pixel_sc_b: SetPixelShaderConstantBFn,
    ) -> HookDirect3D9Device {
        HookDirect3D9Device {
            real_draw_indexed_primitive: real_draw_indexed_primitive,
            //real_begin_scene: real_begin_scene,
            real_release: real_release,
            real_present: real_present,
            real_set_texture: real_set_texture,
            real_set_vertex_sc_f: real_set_vertex_sc_f,
            real_set_vertex_sc_i: real_set_vertex_sc_i,
            real_set_vertex_sc_b: real_set_vertex_sc_b,
            real_set_pixel_sc_f: real_set_pixel_sc_f,
            real_set_pixel_sc_i: real_set_pixel_sc_i,
            real_set_pixel_sc_b: real_set_pixel_sc_b,

            ref_count: 0,
        }
    }
}
// ===========================================================================
// D3D11

pub struct HookDirect3D911Context {
    pub real_draw: DrawFn,
    pub real_draw_auto: DrawAutoFn,
    pub real_draw_indexed: DrawIndexedFn,
    pub real_draw_instanced: DrawInstancedFn,
    pub real_draw_indexed_instanced: DrawIndexedInstancedFn,
    pub real_draw_instanced_indirect: DrawInstancedIndirectFn,
    pub real_draw_indexed_instanced_indirect: DrawIndexedInstancedIndirectFn,
}