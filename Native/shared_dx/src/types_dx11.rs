use crate::defs_dx11::*;

#[derive(Clone, Copy)]
pub struct HookDirect3D11Device {
    pub real_create_buffer: CreateBufferFn,
    pub real_create_texture_2d: CreateTexture2DFn,
    pub real_query_interface: QueryInterfaceFn,
    pub real_create_input_layout: CreateInputLayoutFn
}
#[derive(Clone, Copy)]
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
    // The real fns behind the dynamic buffer hooks, so only present with
    // `snapshot-dynamic-buffers`.  This struct is `Copy` and is returned by value from
    // `get_hook_context()` on every draw and IA state call, so keep the default build's copy
    // exactly the size it was before the feature existed.
    // JMQ: not sure whether the size of this structure is really a perf factor or not, but its on the verge already of 
    // tipping over 128 bytes and I am concerned LLVM may drop some related optimization when it exceeds that.
    #[cfg(feature = "snapshot-dynamic-buffers")]
    pub real_map: MapFn,
    #[cfg(feature = "snapshot-dynamic-buffers")]
    pub real_unmap: UnmapFn,
    #[cfg(feature = "snapshot-dynamic-buffers")]
    pub real_update_subresource: UpdateSubresourceFn,
}
#[derive(Clone, Copy)]
pub struct HookDirect3D11 {
    pub context: HookDirect3D11Context,
}