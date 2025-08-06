cbuffer LightingBuffer : register(b1)
{
    float3 lightDirection; // Should be normalized in app code
    float  padding;        // For alignment (16-byte row)
};

struct VS_OUTPUT
{
    float4 position : SV_POSITION;
    float3 normal   : NORMAL;  // Unpacked and passed to PS
};

float4 main(VS_OUTPUT input) : SV_Target
{
    float3 N = normalize(input.normal);
    float3 L = normalize(-lightDirection); // Negated so light "comes from" the direction

    float diffuse = saturate(dot(N, L));
    float ambient = 0.2;

    float brightness = ambient + diffuse * 0.8;
    return float4(brightness, brightness, brightness, 1.0); // grayscale lit color

    //return float4(1.0, 0.0, 0.0, 1.0); // bright red
}
