use crate::defs_dx11::*;

pub struct HookDirect3D911Context { // TODO11: need ref_count field? d3d9 has one
    pub real_release: IUnknownReleaseFn,
    pub real_vs_setconstantbuffers: VSSetConstantBuffersFn,
    pub real_draw: DrawFn,
    pub real_draw_auto: DrawAutoFn,
    pub real_draw_indexed: DrawIndexedFn,
    pub real_draw_instanced: DrawInstancedFn,
    pub real_draw_indexed_instanced: DrawIndexedInstancedFn,
    pub real_draw_instanced_indirect: DrawInstancedIndirectFn,
    pub real_draw_indexed_instanced_indirect: DrawIndexedInstancedIndirectFn,
}