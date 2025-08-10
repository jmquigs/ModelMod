use glam::{Mat3, Vec2, Vec3};

use glam::{Mat4, Quat};
use winapi::um::d3d11::{ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11SamplerState, D3D11_COMPARISON_NEVER, D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_FLOAT32_MAX, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_WRAP};

use crate::d3d11_utilfn::{create_constant_buffer, update_constant_buffer};

pub struct RendData {
    pub mvp_buf: *mut ID3D11Buffer,
    pub light_buf: *mut ID3D11Buffer,
    pub normal_mat_buf: *mut ID3D11Buffer,
    pub glob_const_buf: *mut ID3D11Buffer,
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct MatrixBuffer {
    pub mvp: [[f32; 4]; 4], // Mat4 in column-major order
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct MatrixBufferMat3 {
    pub mat: [[f32; 3]; 3], // Mat3 in column-major order
    pub _padding: [f32; 3],    // Pad to 48 bytes (3 columns * 16 bytes)
}

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct LightingBuffer {
    pub light_direction: [f32; 3],
    pub _padding: f32, // ensure 16-byte alignment
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct GlobalConstants {
    use_texture: i32,          // HLSL bool = 4 bytes, use i32 here
    _padding: [f32; 3],         // pad to 16 bytes total
}


pub unsafe fn create_data(device:*mut ID3D11Device) -> anyhow::Result<RendData> {
    let mvp_buf = create_constant_buffer::<MatrixBuffer>(device)?;
    let light_buf = create_constant_buffer::<LightingBuffer>(device)?;
    let normal_mat_buf = create_constant_buffer::<MatrixBufferMat3>(device)?;
    let global_constants_buf = create_constant_buffer::<GlobalConstants>(device)?;
    Ok(RendData { mvp_buf, light_buf, normal_mat_buf, glob_const_buf: global_constants_buf })
}

pub const ZOOM_MAX:i32 = 10000000;

/// Generates a Model-View-Projection matrix suitable for uploading to a D3D11 constant buffer.
/// Rotation is applied in ZYX order (yaw, pitch, roll).
///
/// # Arguments
/// - `rotation_radians`: Vec3 with rotation around X, Y, Z axes in radians.
/// - `aspect_ratio`: Width / Height of the viewport.
/// - `fov_y_radians`: Vertical field of view in radians.
/// - `z_near`: Near clipping plane.
/// - `z_far`: Far clipping plane.
///
/// # Returns
/// - A 4x4 matrix (`Mat4`) representing the MVP transformation.

#[derive(Copy,Clone)]
pub struct FrustumParams {
    pub aspect_ratio: f32,
    pub fov_y_radians: f32,
    pub z_near: f32,
    pub z_far: f32,
}

pub enum ModelViewParams {
    FixedCam {
        zoom: i32,
        pan: (i16, i16),
        origin: Vec3,
        eye: Vec3,
        rotation_radians: Vec3,
        frustum: FrustumParams,
    },
    OrbitCam {
        orbit_angles: Vec2,   // (yaw, pitch)
        radius: f32,          // orbit “zoom”
        pan: (i16, i16),      // shift-MMB drag
        pivot: Vec3,          // what we’re orbiting around
        model_rotation: Vec3, // object’s own rotation
        frustum: FrustumParams,
    },
}

pub fn generate_mvp_matrix(
    mvp_p:&ModelViewParams,
) -> anyhow::Result<(Mat4,Mat3)> {
    // If mvp_p is not a FixedCam variant, return an error.
    let (zoom, pan, origin, eye, rotation_radians, FrustumParams { aspect_ratio, fov_y_radians, z_near, z_far }) = match mvp_p {
        ModelViewParams::FixedCam { zoom, pan, origin, eye, rotation_radians, frustum } => {
            (*zoom, *pan, *origin, *eye, *rotation_radians, *frustum)
        },
        _ => return Err(anyhow::anyhow!("ModelViewParams is not a FixedCam variant")),
    };

    // Model transform: apply rotation (ZYX)
    let rotation = Quat::from_euler(
        glam::EulerRot::ZYX,
        rotation_radians.z,
        rotation_radians.y,
        rotation_radians.x,
    );

    // Calculate dolly effect by moving the camera along its look axis
    let dolly_distance = zoom as f32 / ZOOM_MAX as f32 * 10.0;
    let look_axis = (origin - eye).normalize();
    let eye_dolly = eye + look_axis * dolly_distance;
    let pan_x = pan.0 as f32 / i16::MAX as f32 * 2.0;
    let pan_y = pan.1 as f32 / i16::MAX as f32 * 2.0;
    let pan_translation = Vec3::new(pan_x, pan_y, 0.0);
    let model = Mat4::from_rotation_translation(rotation, pan_translation);

    let col0 = model.x_axis.truncate(); // Vec3 from first column
    let col1 = model.y_axis.truncate(); // Vec3 from second column
    let col2 = model.z_axis.truncate(); // Vec3 from third column
    let model3x3 = Mat3::from_cols(col0, col1, col2);

    let normal_matrix = model3x3.inverse().transpose();

    let up = Vec3::Y;
    let view = Mat4::look_at_rh(eye_dolly, origin, up); // Use the dolly-adjusted eye position

    // Projection: perspective with correct handedness for D3D
    let proj = Mat4::perspective_rh(fov_y_radians, aspect_ratio, z_near, z_far);

    // Final MVP
    Ok((proj * view * model, normal_matrix))
}

/// Builds the Model-View-Projection and normal matrix for a classic orbit camera.
pub fn generate_mvp_matrix_orbit(
    mvp_p: &ModelViewParams,
) -> anyhow::Result<(Mat4, Mat3)> {
    let (orbit_angles, radius, pan, pivot, model_rotation, FrustumParams { aspect_ratio, fov_y_radians, z_near, z_far }) = match mvp_p {
        ModelViewParams::OrbitCam { orbit_angles, radius, pan, pivot, model_rotation, frustum } => {
            (*orbit_angles, *radius, *pan, *pivot, *model_rotation, *frustum)
        },
        _ => return Err(anyhow::anyhow!("ModelViewParams is not an OrbitCam variant")),
    };
    
    // ───────────────────────────────────────────
    // 1) ORBIT EYE POSITION
    // ───────────────────────────────────────────
    // Clamp pitch to prevent flipping over the poles
    let pitch = orbit_angles.y.clamp(
        -std::f32::consts::FRAC_PI_2 + 0.001,
         std::f32::consts::FRAC_PI_2 - 0.001,
    );
    let yaw = orbit_angles.x;
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();

    // Eye in world space (spherical → Cartesian)
    let mut eye = Vec3::new(
        radius * cp * sy,  // X
        radius * sp,       // Y
        radius * cp * cy,  // Z
    ) + pivot;

    // ───────────────────────────────────────────
    // 2) OPTIONAL PAN (shift + middle drag)
    // ───────────────────────────────────────────
    let pan_x = pan.0 as f32 / i16::MAX as f32 * 2.0;
    let pan_y = - pan.1 as f32 / i16::MAX as f32 * 2.0;
    let pan_vec = Vec3::new(pan_x, pan_y, 0.0);

    eye   += pan_vec;
    let pivot_panned = pivot + pan_vec; // keep target under cursor while panning

    // ───────────────────────────────────────────
    // 3) MODEL MATRIX (object space → world)
    // ───────────────────────────────────────────
    let rotation = Quat::from_euler(
        glam::EulerRot::ZYX,
        model_rotation.z,
        model_rotation.y,
        model_rotation.x,
    );
    let model = Mat4::from_rotation_translation(rotation, Vec3::ZERO);

    // Normal matrix (3×3, for lighting)
    let normal_matrix = Mat3::from_mat4(model).inverse().transpose();

    // ───────────────────────────────────────────
    // 4) VIEW & PROJECTION
    // ───────────────────────────────────────────
    let view = Mat4::look_at_rh(eye, pivot_panned, Vec3::Y);
    let proj = Mat4::perspective_rh(fov_y_radians, aspect_ratio, z_near, z_far);

    // ───────────────────────────────────────────
    // 5) MVP
    // ───────────────────────────────────────────
    Ok((proj * view * model, normal_matrix))
}


pub unsafe fn prepare_shader_constants(
    context: *mut ID3D11DeviceContext,
    shape_data:&RendData,
    mvp_p:&ModelViewParams,
    light_dir_world: Vec3,
    has_tex0: bool,
) -> anyhow::Result<()> {
    // 1. Compute the MVP matrix
    let (mvp, normal_mat) = match *mvp_p {
        ModelViewParams::FixedCam { .. } => generate_mvp_matrix(mvp_p)?,
        ModelViewParams::OrbitCam { .. } => generate_mvp_matrix_orbit(mvp_p)?,
    };

    // 2. No transpose needed unless your shader uses 'row_major'
    // If needed, use: let mvp = mvp.transpose();

    let mvp_flat: [f32; 16] = *mvp.as_ref();
    let mut mvp_matrix = [[0.0f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            mvp_matrix[row][col] = mvp_flat[col * 4 + row]; // Column-major
        }
    }

    let mvp_data = MatrixBuffer {
        mvp: mvp_matrix, // [[f32; 4]; 4]
    };

    // Flatten Mat3 (column-major)
    let normal_flat: [f32; 9] = *normal_mat.as_ref();
    let mut normal_matrix = [[0.0f32; 3]; 3];
    for col in 0..3 {
        for row in 0..3 {
            normal_matrix[col][row] = normal_flat[col * 3 + row];
        }
    }

    let normal_data = MatrixBufferMat3 {
        mat: normal_matrix,
        _padding: [0.0; 3],
    };

    // 3. Normalize light direction
    let light = light_dir_world.normalize_or_zero();
    let lighting_data = LightingBuffer {
        light_direction: light.to_array(),
        _padding: 0.0,
    };

    let globconstbuf = GlobalConstants {
        use_texture: if has_tex0 { 1 } else { 0 },
        _padding: [0.0; 3],
    };

    // 4. Upload data to the constant buffers
    update_constant_buffer(context, shape_data.mvp_buf, &mvp_data)?;
    update_constant_buffer(context, shape_data.light_buf, &lighting_data)?;
    update_constant_buffer(context, shape_data.normal_mat_buf, &normal_data)?;
    update_constant_buffer(context, shape_data.glob_const_buf, &globconstbuf)?;

    (*context).VSSetConstantBuffers(
        0, // start slot
        1, // number of buffers
        &shape_data.mvp_buf, // pointer to buffer
    );
    (*context).PSSetConstantBuffers(
        1, // slot 1 matches your shader cbuffer register(b1)
        1,
        &shape_data.light_buf,
    );
    (*context).VSSetConstantBuffers(
        2, // start slot
        1, // number of buffers
        &shape_data.normal_mat_buf, // pointer to buffer
    );

    (*context).PSSetConstantBuffers(3,1, &shape_data.glob_const_buf);

    Ok(())
}

/// return a zeroed buffer of data for the specified number of verts and size; this is used when not using 
/// the "simple vertex" hardcoded format - note since the data is zero and this will be submitted to d3d11 until the 
/// mod loads, it could potentially cause issues on the device (degenerate triangles with coordinates all at zero, etc)
/// probably it would be better to at least put some actual triangles in there.
pub fn get_empty_vertices(vert_size: usize, num_verts: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Calculate the total size needed for the vertices
    let total_size = vert_size * num_verts;
    
    // Create and return a vector of the requested size, filled with zeros
    Ok(vec![0u8; total_size])
}

/// generate a index buffer of up to N indicies, using random indicies.
pub fn get_indices(n:u32) -> Vec<u16> {
    let mut indices = Vec::new();
    for _ in 0..n {
        indices.push(rand::random::<u16>());
    }
    indices
}

pub fn create_texture_sampler(device:*mut ID3D11Device) -> anyhow::Result<*mut ID3D11SamplerState> {
    let desc = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        BorderColor: [0.0; 4],
        MinLOD: 0.0,
        MaxLOD: D3D11_FLOAT32_MAX,
    };
    let mut sampler: *mut ID3D11SamplerState = std::ptr::null_mut();
    unsafe {
        let hr = (*device).CreateSamplerState(&desc, &mut sampler);
        if hr != 0 || sampler.is_null() {
            return Err(anyhow!("create sample state failed: {:X}", hr))
        } else {
            Ok(sampler)
        }
    }
}