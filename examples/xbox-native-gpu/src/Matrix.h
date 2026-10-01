#pragma once
#include <vector>
#include <cmath>
#include <stdexcept>
namespace matrix {
inline void inputs(unsigned n, std::vector<float>& a, std::vector<float>& b) {
    if (!n || n > 256) throw std::runtime_error("Matrix dimension must be 1..256");
    a.resize(n*n); b.resize(n*n);
    for(unsigned i=0;i<n*n;++i) {
        a[i]=float(int((i*17+3)%97)-48)/97.0f;
        b[i]=float(int((i*11+7)%89)-44)/89.0f;
    }
}
inline std::vector<float> reference(unsigned n,const std::vector<float>& a,const std::vector<float>& b) {
    if(a.size()!=n*n || b.size()!=n*n) throw std::runtime_error("Invalid matrix length");
    std::vector<float> c(n*n);
    for(unsigned y=0;y<n;++y) for(unsigned x=0;x<n;++x) {
        float s=0; for(unsigned k=0;k<n;++k) s+=a[y*n+k]*b[k*n+x]; c[y*n+x]=s;
    }
    return c;
}
struct Verification { unsigned mismatches=0; double maxError=0; double checksum=0; };
inline Verification verify(const std::vector<float>& expected,const std::vector<float>& actual) {
    if(expected.size()!=actual.size()) throw std::runtime_error("Invalid result length");
    Verification v;
    for(size_t i=0;i<expected.size();++i) {
        double e=std::abs(double(expected[i])-actual[i]);
        if(!std::isfinite(actual[i]) || e>0.001+0.0002*std::abs(expected[i])) ++v.mismatches;
        if(!std::isfinite(e)) v.maxError=INFINITY;
        else if(e>v.maxError) v.maxError=e;
        v.checksum+=actual[i];
    }
    return v;
}
}
