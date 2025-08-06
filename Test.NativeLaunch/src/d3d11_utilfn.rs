use winapi::{shared::{winerror::SUCCEEDED}, um::d3d11::{ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD, D3D11_USAGE_DYNAMIC}};


pub unsafe fn create_constant_buffer<T>(
    device: *mut ID3D11Device,
) -> anyhow::Result<*mut ID3D11Buffer> {
    let buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<T>() as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER,
        CPUAccessFlags: winapi::um::d3d11::D3D11_CPU_ACCESS_WRITE,
        MiscFlags: 0,
        StructureByteStride: 0,
    };

    let mut buffer_ptr: *mut ID3D11Buffer = std::ptr::null_mut();
    let hr = (*device).CreateBuffer(&buffer_desc, std::ptr::null(), &mut buffer_ptr);
    if SUCCEEDED(hr) {
        Ok(buffer_ptr)
    } else {
        Err(anyhow!("failed to create constant buffer: {:X}", hr))
    }
}

pub unsafe fn update_constant_buffer<T: Copy>(
    context: *mut ID3D11DeviceContext,
    buffer: *mut ID3D11Buffer,
    data: &T,
) -> anyhow::Result<()> {
    let mut mapped = std::mem::zeroed::<D3D11_MAPPED_SUBRESOURCE>();
    let hr = (*context).Map(
        buffer as *mut _,
        0,
        D3D11_MAP_WRITE_DISCARD,
        0,
        &mut mapped,
    );

    if SUCCEEDED(hr) {
        std::ptr::copy_nonoverlapping(
            data as *const T as *const u8,
            mapped.pData as *mut u8,
            std::mem::size_of::<T>(),
        );
        (*context).Unmap(buffer as *mut _, 0);
        Ok(())
    } else {
        Err(anyhow!("failed to update constant buffer: {:X}", hr))
    }
}