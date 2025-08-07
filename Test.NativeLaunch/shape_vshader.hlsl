cbuffer MatrixBuffer : register(b0)
{
    matrix modelViewProjection;
};

cbuffer NormalMatrixBuffer : register(b2)
{
    float3x3 normalMatrix;
};

struct VS_INPUT
{
    float3 position             : POSITION;
    int4   normal               : NORMAL;       // Packed as R16G16B16A16_SINT
    int2   binormal             : BINORMAL;     // Ignored for now
    float4 texturecoordinate0   : TEXCOORD0;
};

struct VS_OUTPUT
{
    float4 position           : SV_POSITION;
    float3 normal             : NORMAL;
    float2 texturecoordinate0 : TEXCOORD0;
};

VS_OUTPUT main(VS_INPUT input)
{
    VS_OUTPUT output;

    output.position = mul(float4(input.position, 1.0), modelViewProjection);

    float3 normal;
    normal.x = input.normal.x / 32767.0;
    normal.y = input.normal.y / 32767.0;
    normal.z = input.normal.z / 32767.0;
    normal = normalize(normal);

    normal = mul(normalMatrix, normal);

    output.normal = normalize(normal);
    output.texturecoordinate0.xy = input.texturecoordinate0;

    return output;
}
