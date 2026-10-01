#include "../src/Matrix.h"
#include <cassert>
#include <limits>
#include <iostream>
int main(){
 auto c=matrix::reference(2,{1,2,3,4},{5,6,7,8});
 assert((c==std::vector<float>{19,22,43,50}));
 std::vector<float> ones(17*17,1.0f);
 auto all=matrix::reference(17,ones,ones);
 for(float x:all)assert(x==17.0f);
 assert(matrix::verify(c,c).mismatches==0);
 auto bad=c;bad[3]+=1;assert(matrix::verify(c,bad).mismatches==1);
 bad=c;bad[0]=std::numeric_limits<float>::quiet_NaN();assert(matrix::verify(c,bad).mismatches==1);
 bad=c;bad[0]=std::numeric_limits<float>::infinity();assert(matrix::verify(c,bad).mismatches==1);
 bool rejected=false;try{matrix::verify(c,{1});}catch(const std::runtime_error&){rejected=true;}assert(rejected);
 std::vector<float>a,b;matrix::inputs(256,a,b);assert(a.size()==65536&&b.size()==65536);
 for(float x:a)assert(std::isfinite(x)&&std::abs(x)<=0.5f);
 rejected=false;try{matrix::inputs(257,a,b);}catch(const std::runtime_error&){rejected=true;}assert(rejected);
 std::cout<<"CPU reference fixtures, nonfinite/mismatch detection, and input bounds passed.\n";
}
