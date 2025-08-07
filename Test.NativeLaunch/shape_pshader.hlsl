cbuffer LightingBuffer : register(b1)
{
    float3 lightDirection;
    float  padding;
};
cbuffer MyConstants : register(b3)
{
    bool useTexture;
    float3 _padding2;
};

Texture2D diffuseTexture : register(t0);
SamplerState samplerState : register(s0);

struct VS_OUTPUT
{
    float4 position : SV_POSITION;
    float3 normal   : NORMAL;
    float2 texCoord : TEXCOORD;
};

float4 main(VS_OUTPUT input) : SV_Target
{
    float3 N = normalize(input.normal);
    float3 L = normalize(-lightDirection); // lighting is currently wonky
    float diffuse = saturate(dot(N, L));
    float ambient = 0.2;

    float brightness = ambient + diffuse * 0.8;
    float4 texColor;
    if (useTexture) {
        texColor = diffuseTexture.Sample(samplerState, input.texCoord);
    } else {
        texColor = float4(1,1,1,1);
    }
    return texColor * float4(brightness, brightness, brightness, 1.0);
}
