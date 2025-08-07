use glam::{Mat3, Vec3};

use glam::{Mat4, Quat};
use winapi::um::d3d11::{ID3D11Buffer, ID3D11Device, ID3D11DeviceContext};

use crate::d3d11_utilfn::{create_constant_buffer, update_constant_buffer};

pub struct RendData {
    pub mvp_buf: *mut ID3D11Buffer,
    pub light_buf: *mut ID3D11Buffer,
    pub normal_mat_buf: *mut ID3D11Buffer,
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

pub unsafe fn create_data(device:*mut ID3D11Device) -> anyhow::Result<RendData> {
    let mvp_buf = create_constant_buffer::<MatrixBuffer>(device)?;
    let light_buf = create_constant_buffer::<LightingBuffer>(device)?;
    let normal_mat_buf = create_constant_buffer::<MatrixBufferMat3>(device)?;
    Ok(RendData { mvp_buf, light_buf, normal_mat_buf })
}

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

pub fn generate_mvp_matrix(
    origin: Vec3,
    eye: Vec3,
    rotation_radians: Vec3,
    aspect_ratio: f32,
    fov_y_radians: f32,
    z_near: f32,
    z_far: f32,
) -> (Mat4,Mat3) {
    // Model transform: apply rotation (ZYX)
    let rotation = Quat::from_euler(
        glam::EulerRot::ZYX,
        rotation_radians.z,
        rotation_radians.y,
        rotation_radians.x,
    );
    

    let model = Mat4::from_rotation_translation(rotation, Vec3::ZERO);

    let col0 = model.x_axis.truncate(); // Vec3 from first column
    let col1 = model.y_axis.truncate(); // Vec3 from second column
    let col2 = model.z_axis.truncate(); // Vec3 from third column
    let model3x3 = Mat3::from_cols(col0, col1, col2);

    let normal_matrix = model3x3.inverse().transpose();

    // let model = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)) *
    //         Mat4::from_rotation_y(rotation_radians.y);

    let up = Vec3::Y;
    let view = Mat4::look_at_rh(eye, origin, up);

    // Projection: perspective with correct handedness for D3D
    let proj = Mat4::perspective_rh(fov_y_radians, aspect_ratio, z_near, z_far);

    // Final MVP
    (proj * view * model, normal_matrix)
}

pub unsafe fn prepare_shader_constants(
    context: *mut ID3D11DeviceContext,
    shape_data:&RendData,
    origin: Vec3,
    eye: Vec3,    
    rotation: Vec3,
    aspect_ratio: f32,
    fov_y_radians: f32,
    z_near: f32,
    z_far: f32,
    light_dir_world: Vec3,
) -> anyhow::Result<()> {
    // 1. Compute the MVP matrix
    let (mvp, normal_mat) = generate_mvp_matrix(origin, eye, rotation, aspect_ratio, fov_y_radians, z_near, z_far);

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

    // 4. Upload data to the constant buffers
    update_constant_buffer(context, shape_data.mvp_buf, &mvp_data)?;
    update_constant_buffer(context, shape_data.light_buf, &lighting_data)?;
    update_constant_buffer(context, shape_data.normal_mat_buf, &normal_data)?;

    (*context).VSSetConstantBuffers(
        0, // start slot
        1, // number of buffers
        &shape_data.mvp_buf, // pointer to buffer
    );
    (*context).VSSetConstantBuffers(
        2, // start slot
        1, // number of buffers
        &shape_data.normal_mat_buf, // pointer to buffer
    );

    (*context).PSSetConstantBuffers(
        1, // slot 1 matches your shader cbuffer register(b1)
        1,
        &shape_data.light_buf,
    );

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
