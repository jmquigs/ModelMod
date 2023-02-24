use crate::defs_dx11::*;

pub struct HookDirect3D911Context {
    pub real_draw: DrawFn,
    pub real_draw_auto: DrawAutoFn,
    pub real_draw_indexed: DrawIndexedFn,
    pub real_draw_instanced: DrawInstancedFn,
    pub real_draw_indexed_instanced: DrawIndexedInstancedFn,
    pub real_draw_instanced_indirect: DrawInstancedIndirectFn,
    pub real_draw_indexed_instanced_indirect: DrawIndexedInstancedIndirectFn,
}