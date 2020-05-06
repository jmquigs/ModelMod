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

}

pub struct DeviceState {
    pub hook_direct3d9: Option<HookDirect3D9>,
    pub hook_direct3d9device: Option<HookDirect3D9Device>,
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
    ) -> HookDirect3D9Device {
        HookDirect3D9Device {
            real_draw_indexed_primitive: real_draw_indexed_primitive,
            //real_begin_scene: real_begin_scene,
            real_release: real_release,
            real_present: real_present,
            real_set_texture: real_set_texture,
            ref_count: 0,
        }
    }
}