use crate::defs_dx9::*;

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
            real_draw_indexed_primitive,
            //real_begin_scene: real_begin_scene,
            real_release,
            real_present,
            real_set_texture,
            real_set_vertex_sc_f,
            real_set_vertex_sc_i,
            real_set_vertex_sc_b,
            real_set_pixel_sc_f,
            real_set_pixel_sc_i,
            real_set_pixel_sc_b,

            ref_count: 0,
        }
    }
}

