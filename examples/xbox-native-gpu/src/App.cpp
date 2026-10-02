#include <collection.h>
#include <windows.h>
#include <wrl/client.h>
#include <d3d11_1.h>
#include <dxgi1_2.h>
#include <dwrite.h>
#include <chrono>
#include <fstream>
#include <sstream>
#include <iomanip>
#include <cstring>
#include <atomic>
#include <thread>
#include "GpuiBridge.h"
#include "Matrix.h"
#include "MatmulShader.h"
using Microsoft::WRL::ComPtr;
using namespace Windows::ApplicationModel::Core;
using namespace Windows::UI::Core;
using namespace Windows::Gaming::Input;
using namespace Windows::Foundation;
using Clock=std::chrono::steady_clock;
static void check(HRESULT h) { if(FAILED(h)) { std::ostringstream s;s<<"HRESULT 0x"<<std::hex<<unsigned(h);throw std::runtime_error(s.str()); } }
static double ms(Clock::time_point a,Clock::time_point b) { return std::chrono::duration<double,std::milli>(b-a).count(); }
static std::wstring wide(const std::string& s) { return std::wstring(s.begin(),s.end()); }
static std::atomic<unsigned> requestedJob{0};
static void requestJob(std::uint32_t n) {
    if(n == 64 || n == 128 || n == 256) requestedJob.store(n);
}
static void gpuiCheck(int result) {
    if(result != 0) throw std::runtime_error(std::string("GPUI: ")+gpui_xbox_last_error());
}
// Read the OS font through DirectWrite, avoiding unrestricted file access in UWP.
static std::vector<unsigned char> systemFont() {
    ComPtr<IDWriteFactory> factory;
    check(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory), reinterpret_cast<IUnknown**>(factory.GetAddressOf())));
    ComPtr<IDWriteFontCollection> collection;check(factory->GetSystemFontCollection(&collection));
    UINT32 index=0;BOOL exists=FALSE;check(collection->FindFamilyName(L"Segoe UI", &index, &exists));
    if(!exists) throw std::runtime_error("Segoe UI is unavailable");
    ComPtr<IDWriteFontFamily> family;check(collection->GetFontFamily(index, &family));
    ComPtr<IDWriteFont> font;check(family->GetFirstMatchingFont(DWRITE_FONT_WEIGHT_NORMAL,DWRITE_FONT_STRETCH_NORMAL,DWRITE_FONT_STYLE_NORMAL,&font));
    ComPtr<IDWriteFontFace> face;check(font->CreateFontFace(&face));
    UINT32 count=0;check(face->GetFiles(&count,nullptr));
    if(count != 1) throw std::runtime_error("Unsupported multi-file system font");
    ComPtr<IDWriteFontFile> file;check(face->GetFiles(&count,file.GetAddressOf()));
    const void* key=nullptr;UINT32 keySize=0;check(file->GetReferenceKey(&key,&keySize));
    ComPtr<IDWriteFontFileLoader> loader;check(file->GetLoader(&loader));
    ComPtr<IDWriteFontFileStream> stream;check(loader->CreateStreamFromKey(key,keySize,&stream));
    UINT64 length=0;check(stream->GetFileSize(&length));
    if(!length || length>32*1024*1024) throw std::runtime_error("Invalid system font size");
    const void* data=nullptr;void* fragment=nullptr;check(stream->ReadFileFragment(&data,0,length,&fragment));
    std::vector<unsigned char> bytes;
    try { const auto* begin=static_cast<const unsigned char*>(data);bytes.assign(begin,begin+static_cast<size_t>(length)); }
    catch(...) { stream->ReleaseFileFragment(fragment);throw; }
    stream->ReleaseFileFragment(fragment);return bytes;
}
namespace XboxGpu {
ref class App sealed : public IFrameworkView {
internal:
    App() {}
public:
    virtual void Initialize(CoreApplicationView^ view) {
        view->Activated += ref new TypedEventHandler<CoreApplicationView^,Windows::ApplicationModel::Activation::IActivatedEventArgs^>(this,&App::Activated);
    }
    virtual void SetWindow(CoreWindow^ window) {
        window_=window;
        window->Closed += ref new TypedEventHandler<CoreWindow^,CoreWindowEventArgs^>(this,&App::Closed);
        window->VisibilityChanged += ref new TypedEventHandler<CoreWindow^,VisibilityChangedEventArgs^>(this,&App::Visibility);
        window->SizeChanged += ref new TypedEventHandler<CoreWindow^,WindowSizeChangedEventArgs^>(this,&App::Resize);
        window->CharacterReceived += ref new TypedEventHandler<CoreWindow^,CharacterReceivedEventArgs^>(this,&App::Character);
        window->KeyDown += ref new TypedEventHandler<CoreWindow^,KeyEventArgs^>(this,&App::Key);
    }
    virtual void Load(Platform::String^) {}
    virtual void Uninitialize() { if(uiReady_) { gpui_xbox_shutdown();uiReady_=false; } }
    virtual void Run() {
        try {
            initCompute();
            auto font=systemFont();auto bounds=window_->Bounds;
            gpuiCheck(gpui_xbox_start(reinterpret_cast<IUnknown*>(window_),bounds.Width,bounds.Height,font.data(),font.size(),&requestJob));
            uiReady_=true;
            gpuiCheck(gpui_xbox_visibility(visible_));
            auto root = Windows::Storage::ApplicationData::Current->LocalFolder->Path;
            gpuiCheck(gpui_xbox_storage(reinterpret_cast<const std::uint16_t*>(root->Data()), root->Length()));
        } catch(const std::exception& e) { fail(e.what());return; }
        catch(Platform::Exception^ e) { fail("Windows initialization failed: "+std::to_string(e->HResult));return; }
        while(!closed_) {
            if(!visible_) { window_->Dispatcher->ProcessEvents(CoreProcessEventsOption::ProcessOneAndAllPending);continue; }
            auto frameStart=Clock::now();
            window_->Dispatcher->ProcessEvents(CoreProcessEventsOption::ProcessAllIfPresent);
            if(closed_) break;
            try {
                auto pads=Gamepad::Gamepads;
                if(pads->Size) {
                    auto buttons=pads->GetAt(0)->GetCurrentReading().Buttons;
                    auto pressed=static_cast<unsigned long long>(buttons)&~previous_;
                    previous_=static_cast<unsigned long long>(buttons);
                    const GamepadButtons masks[]={GamepadButtons::A,GamepadButtons::DPadLeft,GamepadButtons::DPadRight,GamepadButtons::DPadUp,GamepadButtons::DPadDown,GamepadButtons::X,GamepadButtons::Y,GamepadButtons::LeftShoulder,GamepadButtons::B};
                    const GpuiKey codes[]={GpuiKey::Run,GpuiKey::Left,GpuiKey::Right,GpuiKey::Up,GpuiKey::Down,GpuiKey::Medium,GpuiKey::Small,GpuiKey::SwitchTool,GpuiKey::Cancel};
                    for(unsigned i=0;i<9;++i) if(pressed&static_cast<unsigned long long>(masks[i])) gpuiCheck(gpui_xbox_key(codes[i]));
                } else previous_=0;
                // Present the busy state before the synchronous native workload.
                gpuiCheck(gpui_xbox_frame());
                auto n=requestedJob.exchange(0);
                if(n && shader_) runJob(n);
            } catch(const std::exception& e) { fail(e.what());requestedJob.store(0); }
            catch(Platform::Exception^ e) { fail("Windows operation failed: "+std::to_string(e->HResult));closed_=true; }
            // GPUI may skip unchanged frames; avoid spinning at full CPU then.
            auto elapsed=Clock::now()-frameStart;
            if(elapsed<std::chrono::milliseconds(16)) std::this_thread::sleep_for(std::chrono::milliseconds(16)-elapsed);
        }
        gpui_xbox_shutdown();uiReady_=false;
    }
private:
    CoreWindow^ window_;
    bool closed_=false,visible_=true,uiReady_=false;
    unsigned long long previous_=0;
    ComPtr<ID3D11Device> device_;
    ComPtr<ID3D11DeviceContext> context_;
    ComPtr<ID3D11ComputeShader> shader_;
    void Activated(CoreApplicationView^,Windows::ApplicationModel::Activation::IActivatedEventArgs^) {window_->Activate();}
    void Closed(CoreWindow^,CoreWindowEventArgs^) {closed_=true;}
    void Visibility(CoreWindow^,VisibilityChangedEventArgs^ e) {
        visible_=e->Visible;
        if(uiReady_ && gpui_xbox_visibility(visible_) != 0) { save("error.txt",gpui_xbox_last_error());closed_=true; }
    }
    void Resize(CoreWindow^,WindowSizeChangedEventArgs^ e) {
        if(uiReady_ && gpui_xbox_resize(e->Size.Width,e->Size.Height) != 0) { save("error.txt",gpui_xbox_last_error());closed_=true; }
    }
    void Character(CoreWindow^, CharacterReceivedEventArgs^ e) {
        if(uiReady_ && gpui_xbox_character(e->KeyCode) != 0) { save("error.txt",gpui_xbox_last_error());closed_=true; }
    }
    void Key(CoreWindow^,KeyEventArgs^ e) {
        if(!uiReady_) return;
        using Windows::System::VirtualKey;
        unsigned code=0;
        switch(e->VirtualKey) {
        case VirtualKey::Enter: code=GpuiKey::Run;break;
        case VirtualKey::F2: case VirtualKey::X: code=GpuiKey::Medium;break;
        case VirtualKey::Y: code=GpuiKey::Small;break;
        case VirtualKey::Left: code=GpuiKey::Left;break;
        case VirtualKey::Right: code=GpuiKey::Right;break;
        case VirtualKey::Up: code=GpuiKey::Up;break;
        case VirtualKey::Down: code=GpuiKey::Down;break;
        case VirtualKey::Tab: code=GpuiKey::Next;break;
        case VirtualKey::F1: code=GpuiKey::SwitchTool;break;
        case VirtualKey::Escape: code=GpuiKey::Cancel;break;
        case VirtualKey::Back: code=GpuiKey::Backspace;break;
        case VirtualKey::Delete: code=GpuiKey::Delete;break;
        default: return;
        }
        e->Handled=true;
        if(gpui_xbox_key(code) != 0) { save("error.txt",gpui_xbox_last_error());closed_=true; }
    }
    bool save(const std::string& name,const std::string& contents) {
        try {
            std::wstring path=Windows::Storage::ApplicationData::Current->LocalFolder->Path->Data();
            path+=L"\\"+wide(name); std::ofstream out(path,std::ios::binary|std::ios::trunc);
            out<<contents;out.flush();return bool(out);
        } catch(...) {return false;}
    }
    void fail(const std::string& reason) {
        save("error.txt",reason);
        OutputDebugStringA(reason.c_str());
        if(!uiReady_ || gpui_xbox_error(reason.c_str()) != 0) closed_=true;
        // Rendering failures are fatal; native workload errors can remain visible.
        if(reason.find("GPUI") != std::string::npos) closed_=true;
    }
    void initCompute() {
        const D3D_FEATURE_LEVEL requested[]={D3D_FEATURE_LEVEL_11_0};D3D_FEATURE_LEVEL actual;
        check(D3D11CreateDevice(nullptr,D3D_DRIVER_TYPE_HARDWARE,nullptr,D3D11_CREATE_DEVICE_BGRA_SUPPORT,requested,1,D3D11_SDK_VERSION,&device_,&actual,&context_));
        check(device_->CreateComputeShader(g_matmulShader,sizeof(g_matmulShader),nullptr,&shader_));
    }
    ComPtr<ID3D11Buffer> buffer(unsigned bytes,unsigned bind,unsigned misc,unsigned stride,const void* data=nullptr,D3D11_USAGE usage=D3D11_USAGE_DEFAULT,unsigned access=0) {
        D3D11_BUFFER_DESC d={};d.ByteWidth=bytes;d.Usage=usage;d.BindFlags=bind;d.MiscFlags=misc;d.StructureByteStride=stride;d.CPUAccessFlags=access;
        D3D11_SUBRESOURCE_DATA init={};init.pSysMem=data;ComPtr<ID3D11Buffer> b;check(device_->CreateBuffer(&d,data?&init:nullptr,&b));return b;
    }
    void runJob(unsigned n) {
        std::vector<float> a,b;matrix::inputs(n,a,b);
        auto t0=Clock::now();auto expected=matrix::reference(n,a,b);auto t1=Clock::now();
        const unsigned count=n*n,bytes=count*sizeof(float);auto g0=Clock::now();
        auto ab=buffer(bytes,D3D11_BIND_SHADER_RESOURCE,D3D11_RESOURCE_MISC_BUFFER_STRUCTURED,4,a.data());
        auto bb=buffer(bytes,D3D11_BIND_SHADER_RESOURCE,D3D11_RESOURCE_MISC_BUFFER_STRUCTURED,4,b.data());
        auto cb=buffer(bytes,D3D11_BIND_UNORDERED_ACCESS,D3D11_RESOURCE_MISC_BUFFER_STRUCTURED,4);
        auto readback=buffer(bytes,0,0,0,nullptr,D3D11_USAGE_STAGING,D3D11_CPU_ACCESS_READ);
        unsigned constants[4]={n,0,0,0};auto params=buffer(16,D3D11_BIND_CONSTANT_BUFFER,0,0,constants);
        D3D11_SHADER_RESOURCE_VIEW_DESC sd={};sd.Format=DXGI_FORMAT_UNKNOWN;sd.ViewDimension=D3D11_SRV_DIMENSION_BUFFER;sd.Buffer.NumElements=count;
        ComPtr<ID3D11ShaderResourceView> av,bv;check(device_->CreateShaderResourceView(ab.Get(),&sd,&av));check(device_->CreateShaderResourceView(bb.Get(),&sd,&bv));
        D3D11_UNORDERED_ACCESS_VIEW_DESC ud={};ud.Format=DXGI_FORMAT_UNKNOWN;ud.ViewDimension=D3D11_UAV_DIMENSION_BUFFER;ud.Buffer.NumElements=count;
        ComPtr<ID3D11UnorderedAccessView> cv;check(device_->CreateUnorderedAccessView(cb.Get(),&ud,&cv));
        ID3D11ShaderResourceView* srvs[]={av.Get(),bv.Get()};ID3D11UnorderedAccessView* uavs[]={cv.Get()};ID3D11Buffer* cbs[]={params.Get()};
        context_->CSSetShader(shader_.Get(),nullptr,0);context_->CSSetShaderResources(0,2,srvs);context_->CSSetUnorderedAccessViews(0,1,uavs,nullptr);context_->CSSetConstantBuffers(0,1,cbs);
        context_->Dispatch((n+15)/16,(n+15)/16,1);
        ID3D11UnorderedAccessView* nullUav[]={nullptr};ID3D11ShaderResourceView* nullSrv[]={nullptr,nullptr};ID3D11Buffer* nullCb[]={nullptr};
        context_->CSSetUnorderedAccessViews(0,1,nullUav,nullptr);context_->CSSetShaderResources(0,2,nullSrv);context_->CSSetConstantBuffers(0,1,nullCb);
        context_->CopyResource(readback.Get(),cb.Get());D3D11_MAPPED_SUBRESOURCE mapped={};
        check(context_->Map(readback.Get(),0,D3D11_MAP_READ,0,&mapped));
        std::vector<float> result(count);std::memcpy(result.data(),mapped.pData,bytes);context_->Unmap(readback.Get(),0);
        auto g1=Clock::now();auto verification=matrix::verify(expected,result);
        double cpu=ms(t0,t1),gpu=ms(g0,g1);auto limit=Windows::System::MemoryManager::AppMemoryUsageLimit;
        std::ostringstream json;json<<std::setprecision(12)<<"{\n  \"version\": 1,\n  \"backend\": \"native D3D11 hardware cs_5_0\",\n  \"workload\": \"C=A*B row-major float32\",\n  \"n\": "<<n<<",\n  \"verified\": "<<(verification.mismatches?"false":"true")<<",\n  \"mismatches\": "<<verification.mismatches<<",\n  \"cpuMs\": "<<cpu<<",\n  \"gpuTotalMs\": "<<gpu<<",\n  \"appMemoryLimitBytes\": "<<limit<<",\n  \"timingScope\": \"one sample; CPU scalar reference; GPU allocation, upload, dispatch, sync and readback; shader creation excluded\"\n}\n";
        bool saved=save("native-gpu-result.json",json.str());
        gpuiCheck(gpui_xbox_result(n,cpu,gpu,verification.maxError,limit,static_cast<std::uint32_t>(verification.mismatches),saved));
    }
};
ref class Source sealed : public IFrameworkViewSource { public: virtual IFrameworkView^ CreateView(){return ref new App();} };
} // namespace XboxGpu
[Platform::MTAThread]
int main(Platform::Array<Platform::String^>^) {CoreApplication::Run(ref new XboxGpu::Source());return 0;}
