#pragma once
#include <cstddef>
#include <cstdint>
// All functions run on the CoreWindow thread; shutdown precedes window release.
extern "C" {
int gpui_xbox_start(void* coreWindow, float width, float height,
                    const unsigned char* font, std::size_t length,
                    void (*requestJob)(std::uint32_t));
int gpui_xbox_frame();
int gpui_xbox_storage(const std::uint16_t* path, std::size_t length);
int gpui_xbox_character(std::uint32_t code);
int gpui_xbox_key(std::uint32_t code);
int gpui_xbox_command(std::uint32_t code);
int gpui_xbox_keyboard(std::uint32_t virtualKey, std::uint32_t modifiers);
int gpui_xbox_activate();
int gpui_xbox_scroll(std::int32_t direction);
int gpui_xbox_resize(float width, float height);
int gpui_xbox_visibility(int visible);
int gpui_xbox_result(std::uint32_t n, double cpu, double gpu, double maxError,
                     std::uint64_t memory, std::uint32_t mismatches, int saved);
int gpui_xbox_error(const char* message);
const char* gpui_xbox_last_error();
int gpui_xbox_shutdown();
}
enum GpuiKey : std::uint32_t { Run = 1, Left, Right, Up, Down, Medium, Small, Next, SwitchTool, Cancel, Backspace, Delete };
