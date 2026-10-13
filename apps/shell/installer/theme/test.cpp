// SPDX-License-Identifier: GPL-3.0-or-later
// Exercises real Win32 controls without installing Iris or displaying a wizard.
#include "theme.cpp"
#include <cstdio>
#include <cstdlib>

namespace {
int clicks = 0;
PluginCallback cleanup = nullptr;
void require(bool ok, const char* message) {
  if (!ok) { std::fprintf(stderr, "%s\n", message); std::exit(1); }
}
int __stdcall registerCallback(HMODULE, PluginCallback callback) { cleanup = callback; return 0; }
LRESULT CALLBACK hostProc(HWND window, UINT msg, WPARAM w, LPARAM l) {
  if (msg == WM_COMMAND && LOWORD(w) == IDOK && HIWORD(w) == BN_CLICKED) ++clicks;
  return DefWindowProcW(window, msg, w, l);
}
COLORREF pixel(HWND window, int x, int y) {
  HDC screen = GetDC(window), dc = CreateCompatibleDC(screen);
  HBITMAP bitmap = CreateCompatibleBitmap(screen, 200, 40);
  HGDIOBJ previous = SelectObject(dc, bitmap);
  SendMessageW(window, WM_PRINTCLIENT, reinterpret_cast<WPARAM>(dc), PRF_CLIENT);
  COLORREF result = GetPixel(dc, x, y);
  SelectObject(dc, previous);
  DeleteObject(bitmap); DeleteDC(dc); ReleaseDC(window, screen);
  return result;
}
}
int main() {
  INITCOMMONCONTROLSEX controls{sizeof(controls), ICC_PROGRESS_CLASS};
  require(InitCommonControlsEx(&controls) != FALSE, "common controls");
  WNDCLASSW klass{};
  klass.lpfnWndProc = hostProc;
  klass.hInstance = GetModuleHandleW(nullptr);
  klass.lpszClassName = L"IrisThemeTest";
  require(RegisterClassW(&klass) != 0, "register test window");
  HWND parent = CreateWindowW(klass.lpszClassName, L"", WS_OVERLAPPED, 0, 0, 400, 300,
    nullptr, nullptr, klass.hInstance, nullptr);
  require(parent != nullptr, "create test window");
  HWND button = CreateWindowW(L"Button", L"Next", WS_CHILD | BS_PUSHBUTTON, 0, 0, 200, 40,
    parent, reinterpret_cast<HMENU>(IDOK), klass.hInstance, nullptr);
  HWND checkbox = CreateWindowW(L"Button", L"Run", WS_CHILD | BS_AUTOCHECKBOX, 0, 50, 200, 40,
    parent, reinterpret_cast<HMENU>(10), klass.hInstance, nullptr);
  HWND progress = CreateWindowW(PROGRESS_CLASSW, L"", WS_CHILD, 0, 100, 200, 40,
    parent, reinterpret_cast<HMENU>(11), klass.hInstance, nullptr);
  require(button && checkbox && progress, "create native controls");
  Extra extra{}; extra.registerCallback = registerCallback;
  auto* item = static_cast<Stack*>(GlobalAlloc(GPTR, sizeof(Stack) + 32 * sizeof(wchar_t)));
  lstrcpyW(item->text, L"base");
  Apply(parent, 1024, nullptr, &item, &extra);
  require(item == nullptr && cleanup != nullptr, "consume NSIS stack and register lifecycle callback");
  require(pixel(button, 20, 10) == color(accent, COLOR_HIGHLIGHT), "primary button paint");
  SendMessageW(button, BM_CLICK, 0, 0);
  require(clicks == 1, "native button command preserved");
  SendMessageW(checkbox, BM_CLICK, 0, 0);
  require(SendMessageW(checkbox, BM_GETCHECK, 0, 0) == BST_CHECKED, "native checkbox toggle preserved");
  SendMessageW(checkbox, BM_CLICK, 0, 0);
  require(SendMessageW(checkbox, BM_GETCHECK, 0, 0) == BST_UNCHECKED, "native checkbox untoggle preserved");
  EnableWindow(button, FALSE);
  require(!IsWindowEnabled(button), "disabled state preserved");
  require(pixel(button, 20, 10) == color(RGB(40, 40, 44), COLOR_BTNFACE), "disabled paint");
  SendMessageW(progress, PBM_SETRANGE32, 20, 120);
  SendMessageW(progress, PBM_SETPOS, 70, 0);
  require(pixel(progress, 30, 20) == color(accent, COLOR_HIGHLIGHT), "progress fill uses native range");
  require(pixel(progress, 150, 20) == color(inset, COLOR_BTNFACE), "progress track");
  cleanup(1); cleanup(0);
  DWORD_PTR data = 0;
  require(!GetWindowSubclass(button, paintProc, subclassId, &data), "callback removes subclasses");
  require(!backgroundBrush && !insetBrush && !headingFont, "callback releases GDI resources");
  DestroyWindow(parent);
  std::puts("Installer theme: native commands, checkbox, disabled paint, progress and cleanup passed.");
}
