use crate::defs_dx11::*;

pub struct HookDirect3D11Device {
    pub real_query_interface: QueryInterfaceFn,
    pub real_create_input_layout: CreateInputLayoutFn
}
pub struct HookDirect3D11Context {
    pub real_query_interface: QueryInterfaceFn,
    pub real_release: IUnknownReleaseFn,
    pub real_vs_setconstantbuffers: VSSetConstantBuffersFn,
    pub real_draw: DrawFn,
    pub real_draw_auto: DrawAutoFn,
    pub real_draw_indexed: DrawIndexedFn,
    pub real_draw_instanced: DrawInstancedFn,
    pub real_draw_indexed_instanced: DrawIndexedInstancedFn,
    pub real_draw_instanced_indirect: DrawInstancedIndirectFn,
    pub real_draw_indexed_instanced_indirect: DrawIndexedInstancedIndirectFn,
    pub real_ia_set_vertex_buffers: IASetVertexBuffersFn,
    pub real_ia_set_input_layout: IASetInputLayoutFn,
    pub real_ia_set_primitive_topology: IASetPrimitiveTopologyFn,
    pub real_ps_set_shader_resources: PSSetShaderResourcesFn,
}
pub struct HookDirect3D11 {
    pub context: HookDirect3D11Context,
}