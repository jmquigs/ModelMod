struct VS_INPUT
{
    float3 position             : POSITION;
    int4     normal               : NORMAL;
    int2   binormal             : BINORMAL;
    float4    texturecoordinate0   : TEXCOORD0;
};

struct VS_OUTPUT
{
    float4 position : SV_POSITION;
};

VS_OUTPUT main(VS_INPUT input)
{
    VS_OUTPUT output;
    output.position = float4(input.position, 1.0);
    return output;
}
