use crate::mod_vector::Float3;

/// This file contains (mostly) LLM-generated encoding and decoding functions for vector formats.
/// It is the rust variant of the `DataEncoding.fs` in the managed code.

fn to_frac15(x: f32) -> i32 {
    const ONE_OVER_TWO: f32 = 0.5;
    const FRAC_SCALE: f32 = 32767.0;

    let clamped = x.max(-1.0).min(1.0);
    ((clamped * ONE_OVER_TWO + ONE_OVER_TWO) * FRAC_SCALE).round() as i32
}

/// LLM-generated (o3) normal encoding fn
pub fn encode_packed_vector(v: &Float3) -> (i16, i16) {
    const SHIFT: i32 = 32768;

    let sign_bit = if v.z >= 0.0 { 1 } else { 0 };
    let frac_x = to_frac15(v.x);
    let frac_y = to_frac15(v.y);

    let raw0 = sign_bit * SHIFT + frac_x - SHIFT;
    let raw1 = frac_y - SHIFT;

    (raw0 as i16, raw1 as i16)
}

fn decode_component(raw: i16) -> (f32, i32) {
    // Shift from [-32768, 32767] -> [0, 65535], then normalize by 32768
    let u = (raw as f32 + 32768.0) * (1.0 / 32768.0); // 0..<2
    let sign_bit = if u >= 1.0 { 1 } else { 0 };

    // Fractional part holds the x or y value, scale to -1...+1
    let frac = u - sign_bit as f32;
    let fcomponent = frac * 2.0 - 1.0;

    (fcomponent, sign_bit)
}

pub fn decode_packed_vector(a: i16, b: i16) -> (f32, f32, f32) {
    let (x, sign_z_bit) = decode_component(a);
    let (y, _) = decode_component(b); // sign bit of Y is unused

    // Reconstruct Z so that |N| = 1
    let z_len_squared = (1.0 - (x * x + y * y)).max(0.0);
    let z_magnitude = z_len_squared.sqrt();
    let z = if sign_z_bit == 0 { -z_magnitude } else { z_magnitude };

    (x, y, z)
}

/// Another LLM Generated normal encoding.  Two LLMs (4o and o3) both say this
/// more closely matches the sample shader I gave them, but when I try to use it
/// the results are much worse (in particular it seems to flip the handedness
/// of the coordinate system on a per-vert basis as shown by the debug )
pub fn encode_octa_vector(v: &Float3) -> (i16, i16) {
    //--- 1. normalise (defensive) ------------------------------------------
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    // Note this z was mut in the original code the LLM generated, but compiler says it doesn't need to be; evidence of a problem?
    let (mut x, mut y, z) = (v.x / len, v.y / len, v.z / len);

    //--- 2. fold the lower hemisphere onto the upper one -------------------
    if z < 0.0 {
        let absx = x.abs();
        let absy = y.abs();
        x = (1.0 - absy) * x.signum();
        y = (1.0 - absx) * y.signum();
    }

    //--- 3. map from [-1,1] to [0,1] and bake sign(z) in the integer part --
    const SHIFT: i32 = 32768;               // 2^15
    const FRAC_SCALE: f32 = 32767.0;        // keeps 0x7FFF free for the sign

    let to_frac15 = |c: f32| -> i32 {
        (((c * 0.5 + 0.5) * FRAC_SCALE).round()) as i32
    };

    let sign_bit = if v.z >= 0.0 { 0 } else { 1 };   // 0 or 1
    let raw0 = sign_bit * SHIFT + to_frac15(x) - SHIFT;
    let raw1 =                /* sign bit not stored here */ to_frac15(y) - SHIFT;

    (raw0 as i16, raw1 as i16)
}

/// See encode_octa for more info about this
pub fn decode_octa_vector(a: i16, b: i16) -> (f32, f32, f32) {
    // 1) pull integer sign bit and fractional part (your original code)
    let u0 = (a as f32 + 32768.0) * (1.0 / 32768.0);   // 0‥<2
    let u1 = (b as f32 + 32768.0) * (1.0 / 32768.0);

    let sign_z = if u0 >= 1.0 { 1.0 } else { -1.0 };
    let mut x  = if sign_z > 0.0 { u0 - 1.0 } else { u0 }; // frac in [0,1)
    let mut y  =                if u1 >= 1.0 { u1 - 1.0 } else { u1 };

    x = x * 2.0 - 1.0;   // map back to [-1,1]
    y = y * 2.0 - 1.0;

    // 2) *** UNFOLD the lower hemisphere ***
    if sign_z < 0.0 {
        let ax = x.abs();
        let ay = y.abs();
        let old_x = x;
        x = (1.0 - ay) * old_x.signum();
        y = (1.0 - ax) * y.signum();
    }

    // 3) reconstruct z so that |N| = 1
    let z_len2 = (1.0 - (x * x + y * y)).max(0.0);
    let z = z_len2.sqrt() * sign_z;

    (x, y, z)
}
