// Bounded, native compute: C = A * B. Row-major float32 matrices.
StructuredBuffer<float> A : register(t0);
StructuredBuffer<float> B : register(t1);
RWStructuredBuffer<float> C : register(u0);
cbuffer Parameters : register(b0) { uint N; uint3 padding; };
[numthreads(16, 16, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x >= N || id.y >= N) return;
    float sum = 0.0;
    for (uint k = 0; k < N; ++k) sum += A[id.y * N + k] * B[k * N + id.x];
    C[id.y * N + id.x] = sum;
}
