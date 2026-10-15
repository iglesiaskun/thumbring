// Ring shapes drawn as signed distance fields with a smooth union.
//
// Distances are in physical pixels, so the 1 px antialiasing needs no DPI
// factor. The output is premultiplied, as the composition swap chain expects.

cbuffer Scene : register(b0)
{
    float4 white[32];     // x, y, radius, unused
    float4 dark[24];
    float4 centerCircle;  // x, y, radius, red (0 to 1)
    float4 blob;          // x, y, radius, on
    float4 blobShape;     // direction x, direction y, stretch, squash
    float4 hero;          // x, y, radius, on
    float4 heroShape;
    float4 darkColor;     // rgb, global alpha
    float4 params;        // white k, dark k, white count, dark count
    float4 params2;       // shadow size, unused
    float4 bounds;        // left, top, right, bottom of the drawn quad
    float4 viewport;      // width, height, unused
};

struct VSOut
{
    float4 pos : SV_Position;
};

// A quad over `bounds`, from the vertex id alone (no vertex buffer).
VSOut vs_main(uint id : SV_VertexID)
{
    float2 corner = float2(id & 1, id >> 1);
    float2 px = lerp(bounds.xy, bounds.zw, corner);
    VSOut o;
    o.pos = float4(px.x / viewport.x * 2.0 - 1.0, 1.0 - px.y / viewport.y * 2.0, 0.0, 1.0);
    return o;
}

float smin(float a, float b, float k)
{
    float h = saturate(0.5 + 0.5 * (b - a) / k);
    return lerp(b, a, h) - k * h * (1.0 - h);
}

// Distance to a circle stretched along a direction (the blob and the hero).
float sdEll(float2 p, float4 c, float4 e)
{
    float2 d = p - c.xy;
    float2 q = float2(dot(d, e.xy), dot(d, float2(-e.y, e.x))) / e.zw;
    return (length(q) - c.z) * min(e.z, e.w);
}

float4 ps_main(VSOut input) : SV_Target
{
    float2 p = input.pos.xy;
    int whiteCount = (int)params.z;
    int darkCount = (int)params.w;

    float dw = 10000.0;
    [loop] for (int i = 0; i < 32; i++)
    {
        if (i >= whiteCount) break;
        float4 c = white[i];
        dw = smin(dw, length(p - c.xy) - c.z, params.x);
    }
    if (hero.w > 0.5) dw = smin(dw, sdEll(p, hero, heroShape), params.x);

    float dd = 10000.0;
    [loop] for (int j = 0; j < 24; j++)
    {
        if (j >= darkCount) break;
        float4 c = dark[j];
        dd = smin(dd, length(p - c.xy) - c.z, params.y);
    }
    if (blob.w > 0.5) dd = smin(dd, sdEll(p, blob, blobShape), params.y);

    // Opacity of the white items and of the center button (white, then red).
    // Almost solid white: lower values turn grey over dark windows.
    const float itemOpacity = 0.96;
    const float centerOpacity = 0.75;
    const float centerRedOpacity = 0.95;

    float aw = saturate(0.5 - dw);
    float shadow = (1.0 - smoothstep(0.0, params2.x, dw)) * 0.10 * (1.0 - aw);
    // The dark layer reaches 1 px past its shape: a dark disc sits exactly on
    // a white item of the same size, and with both edges antialiased the white
    // would show through as a light outline over dark windows.
    float ad = saturate(1.5 - dd);
    float dc = length(p - centerCircle.xy) - centerCircle.z;
    float ac = saturate(0.5 - dc) * lerp(centerOpacity, centerRedOpacity, centerCircle.w);
    float3 cc = lerp(float3(1.0, 1.0, 1.0), float3(0.90, 0.25, 0.33), centerCircle.w);

    float4 col = float4(0.0, 0.0, 0.0, shadow);
    float wa = aw * itemOpacity;
    col = float4(float3(1.0, 1.0, 1.0) * wa, wa) + col * (1.0 - wa);
    col = float4(cc * ac, ac) + col * (1.0 - ac);
    col = float4(darkColor.rgb * ad, ad) + col * (1.0 - ad);
    return col * darkColor.a;
}
