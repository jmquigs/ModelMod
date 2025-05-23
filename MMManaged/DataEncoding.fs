﻿namespace ModelMod

open CoreTypes

open System 

/// Contains various game specific helper utilities for reading/writing mesh data
/// Some of these were originally LLM-generated (openai o3)
module DataEncoding =
    /// Bitwise pack two int16s into a f32
    let packIntoFloat (a:int16) (b:int16):float32 = 
        // Combine the first pair of 16-bit integers (a and b) into a 32-bit integer.
        let combined1 = (uint32 a &&& 0xFFFFu) ||| ((uint32 b &&& 0xFFFFu) <<< 16)
        let packedFloat1 = BitConverter.ToSingle(BitConverter.GetBytes(combined1), 0)
        packedFloat1

    module PackedVectorV1 = 
        /// Decode a single packed component and return (component, signBit)
        let private decodeComponent (raw:int16) : float32 * int =
            // 1) shift from [-32768,32767] -> [0,65535], then normalise by 32768
            let u = (float32 raw + 32768.0f) * (1.0f / 32768.0f)   // 0..<2
            // 2) integer part is either 0 or 1 and stores the sign of Z
            let signBit = if u >= 1.0f then 1 else 0
            // 3) fractional part holds the x or y value, scale to -1...+1
            let frac = u - float32 signBit
            let fcomponent = frac * 2.0f - 1.0f
            fcomponent, signBit

        /// Given two 16-bit words (as in R16G16B16A16_SINT) recover the vector
        let decode (a:int16) (b:int16) : (float32 * float32 * float32) =
            // first normal uses X = word0, Y = word1, sign(Z) also from word0
            let x, signZbit = decodeComponent a
            let y, _        = decodeComponent b   // sign bit in Y is unused

            // reconstruct Z so that |N| = 1
            let zLenSquared = max 0.0f (1.0f - (x*x + y*y))
            let zMagnitude  = sqrt zLenSquared
            let z           = (if signZbit = 0 then -zMagnitude else zMagnitude)

            (x, y, z)            

        [<Literal>] 
        let OneOverTwo   = 0.5f              //  1/2    – scale from [-1..1]→[0..1]
        [<Literal>] 
        let FracScale    = 32767.0f          //  2^15-1 – max fractional value
        [<Literal>] 
        let Shift        = 32768             //  2^15   – bias used by the shader

        /// Convert x ϵ [-1,1] to 15-bit integer fraction ϵ [0, 32767]
        let inline toFrac15 (x:float32) =
            let clamped = max -1.0f (min 1.0f x)
            int (System.Math.Round( float ( (clamped * OneOverTwo + OneOverTwo) * FracScale ) ))

        /// Pack one Vector3 into two int16s: returns (word0, word1)
        let encode (v:Vec3F) : int16 * int16 =
            // sign bit for Z lives in the *integer* part of word0
            let signBit = if v.Z >= 0.0f then 1 else 0       // 0 => negative Z
            let fracX   = toFrac15 v.X
            let fracY   = toFrac15 v.Y

            // raw =  sign*32768  +  frac  - 32768
            let raw0 = signBit * Shift + fracX - Shift
            let raw1 =                 fracY - Shift

            int16 raw0, int16 raw1

    module OctaV1 =
        [<Literal>] 
        let FracScale = 32767.0f        // 2^15-1
        [<Literal>] 
        let Shift     = 32768           // 2^15

        /// +1 for non-negative inputs, –1 for negative
        let inline sgn (x: float32) = if x >= 0.0f then 1.0f else -1.0f

        /// Clamp, map [-1,1] → [0,1], quantise to 15-bit fraction
        let inline toFrac15 (c: float32) =
            let c = max -1.0f (min 1.0f c)
            int (System.Math.Round (float ((c * 0.5f + 0.5f) * FracScale)))

        /// Inverse of `encode`  :  (int16 * int16) → Vec3F
        let decode (w0: int16) (w1: int16) : (float32 * float32 * float32) =
            //--------------------------------------------------------------------
            // 1) pull the “which hemisphere?” flag back out of word‑0
            //--------------------------------------------------------------------
            let signBit   = if   w0 >= 0s then 1 else 0          // 1 ⇒ +Z hemi
            let xQuantInt = if signBit = 1
                            then int w0                          // already 0‥32767
                            else int w0 + Shift                  // bring back to 0‥32767
            let yQuantInt = int w1 + Shift                       // always 0‥32767

            //--------------------------------------------------------------------
            // 2) de‑quantise the two stored components to the (‑1 … +1) range
            //--------------------------------------------------------------------
            let xEnc = ((float32 xQuantInt) / FracScale - 0.5f) * 2.0f
            let yEnc = ((float32 yQuantInt) / FracScale - 0.5f) * 2.0f

            //--------------------------------------------------------------------
            // 3) unfold the lower hemisphere if necessary
            //--------------------------------------------------------------------
            let mutable x, y =
                if signBit = 1 then
                    // original vector was already on +Z side
                    xEnc, yEnc
                else
                    // we had folded it across the XY diagonal in `encode`
                    let sx, sy = sgn xEnc, sgn yEnc
                    sx * (1.0f - abs yEnc),        //  |yOrig| = 1 − |xEnc|
                    sy * (1.0f - abs xEnc)         //  |xOrig| = 1 − |yEnc|

            //--------------------------------------------------------------------
            // 4) recover Z and put the sign back
            //--------------------------------------------------------------------
            let z =
                let zAbs = sqrt (max 0.0f (1.0f - x*x - y*y))   // always ≥ 0
                if signBit = 1 then  zAbs else -zAbs

            //--------------------------------------------------------------------
            // 5) normalise (quantisation & rounding can knock length off slightly)
            //--------------------------------------------------------------------
            let lenInv = 1.0f / sqrt (x*x + y*y + z*z)
            (x*lenInv, y*lenInv, z*lenInv)

        /// Octahedral encode : Vec3 → int16 × int16          (works for N, T, B)
        let encode (v: Vec3F) : int16 * int16 =

            // 1) normalise (defensive)
            let lenInv = 1.0f / sqrt (v.X*v.X + v.Y*v.Y + v.Z*v.Z)
            let mutable x, y, z = v.X * lenInv, v.Y * lenInv, v.Z * lenInv

            // 2) fold lower hemisphere
            if z < 0.0f then
                let ax, ay = abs x, abs y
                x <- (1.0f - ay) * sgn x
                y <- (1.0f - ax) * sgn y
                z <- -z                          // not stored but keeps maths tidy

            // 3) fraction + sign-in-integer-part
            let signBit = if v.Z >= 0.0f then 1 else 0      // 1 ⇒ +Z hemisphere
            let raw0 = signBit * Shift + toFrac15 x - Shift // word0 : sign ⊕ fracX
            let raw1 =                     toFrac15 y - Shift // word1 : fracY

            int16 raw0, int16 raw1

