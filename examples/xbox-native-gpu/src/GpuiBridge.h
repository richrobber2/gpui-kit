#pragma once
#include <cstddef>
#include <cstdint>
// All functions run on the CoreWindow thread; shutdown precedes window release.
extern "C" {
int gpui_xbox_start(void* coreWindow, float width, float height,
                    const unsigned char* font, std::size_t length,
                    void (*requestJob)(std::uint32_t));
int gpui_xbox_frame();
int gpui_xbox_key(std::uint32_t code);
int gpui_xbox_resize(float width, float height);
int gpui_xbox_visibility(int visible);
int gpui_xbox_result(std::uint32_t n, double cpu, double gpu, double maxError,
                     std::uint64_t memory, std::uint32_t mismatches, int saved);
int gpui_xbox_error(const char* message);
const char* gpui_xbox_last_error();
int gpui_xbox_shutdown();
}
enum GpuiKey : std::uint32_t { Run = 1, Left, Right, Up, Down, Medium, Small, Next };
