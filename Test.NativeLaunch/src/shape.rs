use std::i16;
use glam::Vec3;


#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    position: [f32; 3],      // 12 bytes
    normal_tangent: [i16; 4], // 8 bytes (normal.xy, tangent.xy or similar)
    binormal: [i16; 2],      // 4 bytes
    texcoord: [i16; 4],      // 8 bytes (xy used, zw = 0)
}


/// Generates vertex and index buffers for a cube with each face having its own texture coordinates.
///
/// # Returns
/// A tuple of `(vertex_data, index_data)`:
/// - `vertex_data`: `Vec<u8>` containing packed vertex data for D3D11
/// - `index_data`: `Vec<u16>` for 12 triangles (36 indices)
//#[cfg(FALSE)]
pub fn generate_cube_mesh() -> (Vec<u8>, Vec<u16>) {


    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    // Cube face definitions: normal, tangent, binormal, positions, texcoords
    let face_data = [
        // +X
        (Vec3::X, Vec3::Z, Vec3::Y, [[1.0, -1.0, -1.0], [1.0, -1.0,  1.0], [1.0,  1.0,  1.0], [1.0,  1.0, -1.0]]),
        // -X
        (-Vec3::X, Vec3::Z, -Vec3::Y, [[-1.0, -1.0,  1.0], [-1.0, -1.0, -1.0], [-1.0,  1.0, -1.0], [-1.0,  1.0,  1.0]]),
        // +Y
        (Vec3::Y, Vec3::X, Vec3::Z, [[-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]]),
        // -Y
        (-Vec3::Y, Vec3::X, -Vec3::Z, [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, -1.0, -1.0]]),
        // +Z
        (Vec3::Z, Vec3::X, Vec3::Y, [[-1.0, -1.0, 1.0], [-1.0,  1.0, 1.0], [1.0,  1.0, 1.0], [1.0, -1.0, 1.0]]),
        // -Z
        (-Vec3::Z, Vec3::X, -Vec3::Y, [[1.0, -1.0, -1.0], [1.0,  1.0, -1.0], [-1.0,  1.0, -1.0], [-1.0, -1.0, -1.0]]),
    ];

    for (i, (normal, tangent, binormal, positions)) in face_data.iter().enumerate() {
        for (j, pos) in positions.iter().enumerate() {
            let texcoord_f = match j {
                0 => [0.0, 1.0],
                1 => [0.0, 0.0],
                2 => [1.0, 0.0],
                3 => [1.0, 1.0],
                _ => unreachable!(),
            };

            // Pack texcoord to SNORM [-32767, 32767]
            let texcoord = [
                (texcoord_f[0] * 2.0 - 1.0) * 32767.0,
                (texcoord_f[1] * 2.0 - 1.0) * 32767.0,
                0.0,
                0.0,
            ];

            let v = Vertex {
                position: [pos[0], pos[1], pos[2]],
                normal_tangent: [
                    (normal.x * 32767.0) as i16,
                    (normal.y * 32767.0) as i16,
                    (tangent.x * 32767.0) as i16,
                    (tangent.y * 32767.0) as i16,
                ],
                binormal: [
                    (binormal.x * 32767.0) as i16,
                    (binormal.y * 32767.0) as i16,
                ],
                texcoord: [
                    texcoord[0] as i16,
                    texcoord[1] as i16,
                    texcoord[2] as i16,
                    texcoord[3] as i16,
                ],
            };
            vertices.push(v);
        }

        let base = (i * 4) as u16;
        indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base, base + 2, base + 3,
        ]);
    }

    // Convert to byte buffer
    let vertex_bytes = unsafe {
        std::slice::from_raw_parts(
            vertices.as_ptr() as *const u8,
            vertices.len() * std::mem::size_of::<Vertex>(),
        )
    }.to_vec();

    (vertex_bytes, indices)
}

