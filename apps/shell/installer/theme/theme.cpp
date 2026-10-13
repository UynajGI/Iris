// SPDX-License-Identifier: GPL-3.0-or-later
// Iris-owned drawing for existing NSIS controls. No installation operations.
#define NOMINMAX
#include <windows.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <uxtheme.h>
#include <cwchar>
#include <algorithm>

// NSIS Unicode plug-in ABI (https://nsis.sourceforge.io/Plugins).
struct Stack { Stack* next; wchar_t text[1]; };
using PluginCallback = UINT_PTR (__cdecl *)(int);
struct Extra {
  void* flags;
  int (__stdcall *execute)(int, HWND);
  void (__stdcall *validate)(wchar_t*);
  int (__stdcall *registerCallback)(HMODULE, PluginCallback);
};
namespace {
constexpr UINT_PTR subclassId = 0x49524953;
constexpr COLORREF surface = RGB(36, 36, 36);
constexpr COLORREF text = RGB(236, 236, 238);
constexpr COLORREF secondary = RGB(177, 177, 184);
constexpr COLORREF inset = RGB(24, 24, 27);
constexpr COLORREF raised = RGB(49, 49, 54);
constexpr COLORREF accent = RGB(117, 99, 173);
constexpr COLORREF border = RGB(75, 75, 82);
HINSTANCE module = nullptr;
HWND root = nullptr;
HBRUSH backgroundBrush = nullptr, insetBrush = nullptr;
HFONT headingFont = nullptr;
bool highContrast = false;

int scale(HWND window, int value) {
  HDC dc = GetDC(window);
  const int dpi = dc ? GetDeviceCaps(dc, LOGPIXELSY) : 96;
  if (dc) ReleaseDC(window, dc);
  return MulDiv(value, dpi, 96);
}
COLORREF color(COLORREF normal, int system) {
  return highContrast ? GetSysColor(system) : normal;
}
void fill(HDC dc, const RECT& rect, COLORREF value) {
  SetDCBrushColor(dc, value);
  FillRect(dc, &rect, static_cast<HBRUSH>(GetStockObject(DC_BRUSH)));
}
bool isClass(HWND window, const wchar_t* expected) {
  wchar_t name[64]{};
  GetClassNameW(window, name, 64);
  return lstrcmpiW(name, expected) == 0;
}
void drawButton(HWND window, HDC dc) {
  RECT rect{};
  GetClientRect(window, &rect);
  const LONG_PTR style = GetWindowLongPtrW(window, GWL_STYLE);
  const int kind = static_cast<int>(style & BS_TYPEMASK);
  const bool enabled = IsWindowEnabled(window) != FALSE;
  const bool radio = kind == BS_AUTORADIOBUTTON || kind == BS_RADIOBUTTON;
  const bool check = radio || kind == BS_AUTOCHECKBOX || kind == BS_CHECKBOX;
  const bool group = kind == BS_GROUPBOX;
  const bool primary = GetParent(window) == root && GetDlgCtrlID(window) == IDOK;
  const bool pressed = (SendMessageW(window, BM_GETSTATE, 0, 0) & BST_PUSHED) != 0;
  const bool hovered = GetPropW(window, L"IrisHover") != nullptr;
  COLORREF bg = primary ? accent : raised;
  if (pressed) bg = primary ? RGB(103, 85, 155) : RGB(38, 38, 42);
  else if (hovered) bg = primary ? RGB(125, 106, 181) : RGB(60, 60, 66);
  if (!enabled) bg = RGB(40, 40, 44);
  fill(dc, rect, color(surface, COLOR_BTNFACE));
  const int saved = SaveDC(dc);
  SelectObject(dc, reinterpret_cast<HGDIOBJ>(SendMessageW(window, WM_GETFONT, 0, 0)));
  SetBkMode(dc, TRANSPARENT);
  SetTextColor(dc, enabled ? color(text, COLOR_BTNTEXT) : color(RGB(121, 121, 128), COLOR_GRAYTEXT));
  RECT label = rect;
  if (check) {
    const int size = scale(window, 14);
    RECT box{0, (rect.bottom - size) / 2, size, (rect.bottom + size) / 2};
    const bool checked = SendMessageW(window, BM_GETCHECK, 0, 0) != BST_UNCHECKED;
    SelectObject(dc, GetStockObject(DC_PEN));
    SelectObject(dc, GetStockObject(DC_BRUSH));
    SetDCPenColor(dc, color(checked ? accent : RGB(120, 120, 128), COLOR_BTNTEXT));
    SetDCBrushColor(dc, color(checked ? accent : inset, COLOR_BTNFACE));
    if (radio) Ellipse(dc, box.left, box.top, box.right, box.bottom);
    else RoundRect(dc, box.left, box.top, box.right, box.bottom, scale(window, 3), scale(window, 3));
    if (checked) {
      SetDCPenColor(dc, color(RGB(255, 255, 255), COLOR_BTNTEXT));
      SetDCBrushColor(dc, color(RGB(255, 255, 255), COLOR_BTNTEXT));
      if (radio) {
        InflateRect(&box, -scale(window, 4), -scale(window, 4));
        Ellipse(dc, box.left, box.top, box.right, box.bottom);
      } else {
        MoveToEx(dc, scale(window, 3), box.top + scale(window, 7), nullptr);
        LineTo(dc, scale(window, 6), box.top + scale(window, 10));
        LineTo(dc, scale(window, 11), box.top + scale(window, 4));
      }
    }
    label.left += size + scale(window, 9);
  } else if (!group) {
    SelectObject(dc, GetStockObject(DC_PEN));
    SelectObject(dc, GetStockObject(DC_BRUSH));
    SetDCPenColor(dc, color(primary && enabled ? accent : border, COLOR_BTNSHADOW));
    SetDCBrushColor(dc, color(bg, primary && enabled ? COLOR_HIGHLIGHT : COLOR_BTNFACE));
    RoundRect(dc, 0, 0, rect.right, rect.bottom, scale(window, 5), scale(window, 5));
    if (primary && enabled) SetTextColor(dc, color(RGB(255, 255, 255), COLOR_HIGHLIGHTTEXT));
    InflateRect(&label, -scale(window, 5), 0);
  }
  wchar_t caption[1024]{};
  GetWindowTextW(window, caption, 1024);
  UINT format = DT_SINGLELINE | DT_VCENTER | (check || group ? DT_LEFT : DT_CENTER);
  if (style & BS_MULTILINE) format = DT_WORDBREAK | (check || group ? DT_LEFT : DT_CENTER);
  if (SendMessageW(window, WM_QUERYUISTATE, 0, 0) & UISF_HIDEACCEL) format |= DT_HIDEPREFIX;
  DrawTextW(dc, caption, -1, &label, format);
  if (GetFocus() == window && !(SendMessageW(window, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS)) {
    InflateRect(&label, -scale(window, 3), -scale(window, 3));
    DrawFocusRect(dc, &label);
  }
  RestoreDC(dc, saved);
}
void drawProgress(HWND window, HDC dc) {
  RECT rect{};
  GetClientRect(window, &rect);
  fill(dc, rect, color(inset, COLOR_BTNFACE));
  PBRANGE range{};
  SendMessageW(window, PBM_GETRANGE, FALSE, reinterpret_cast<LPARAM>(&range));
  const int position = static_cast<int>(SendMessageW(window, PBM_GETPOS, 0, 0));
  const long long span = static_cast<long long>(range.iHigh) - range.iLow;
  if (span > 0) {
    const long long offset = std::clamp(static_cast<long long>(position) - range.iLow, 0LL, span);
    rect.right = static_cast<LONG>(rect.right * offset / span);
    fill(dc, rect, color(accent, COLOR_HIGHLIGHT));
  }
}
LRESULT CALLBACK paintProc(HWND window, UINT message, WPARAM w, LPARAM l, UINT_PTR, DWORD_PTR) {
  const bool button = isClass(window, L"Button");
  const bool progress = isClass(window, PROGRESS_CLASSW);
  if ((button || progress) && (message == WM_PAINT || message == WM_PRINTCLIENT)) {
    PAINTSTRUCT ps{};
    HDC dc = message == WM_PAINT ? BeginPaint(window, &ps) : reinterpret_cast<HDC>(w);
    if (dc) { if (button) drawButton(window, dc); else drawProgress(window, dc); }
    if (message == WM_PAINT) EndPaint(window, &ps);
    return 0;
  }
  if ((button || progress) && message == WM_ERASEBKGND) return 1;
  if (message == WM_CTLCOLOREDIT || message == WM_CTLCOLORLISTBOX ||
      (message == WM_CTLCOLORSTATIC && isClass(reinterpret_cast<HWND>(l), L"Edit"))) {
    HDC dc = reinterpret_cast<HDC>(w);
    SetTextColor(dc, color(text, COLOR_WINDOWTEXT));
    SetBkColor(dc, color(inset, COLOR_WINDOW));
    return reinterpret_cast<LRESULT>(insetBrush);
  }
  if (message == WM_CTLCOLORDLG) return reinterpret_cast<LRESULT>(backgroundBrush);
  if (button && message == WM_MOUSEMOVE && !GetPropW(window, L"IrisHover")) {
    SetPropW(window, L"IrisHover", reinterpret_cast<HANDLE>(1));
    TRACKMOUSEEVENT tracking{sizeof(tracking), TME_LEAVE, window, 0};
    TrackMouseEvent(&tracking);
    InvalidateRect(window, nullptr, FALSE);
  }
  if (button && message == WM_MOUSELEAVE) {
    RemovePropW(window, L"IrisHover");
    InvalidateRect(window, nullptr, FALSE);
  }
  if (message == WM_NCDESTROY) {
    RemovePropW(window, L"IrisHover");
    RemoveWindowSubclass(window, paintProc, subclassId);
  }
  const LRESULT result = DefSubclassProc(window, message, w, l);
  if (button && (message == WM_SETFOCUS || message == WM_KILLFOCUS || message == WM_ENABLE ||
                 message == BM_SETCHECK || message == BM_SETSTATE || message == WM_UPDATEUISTATE))
    InvalidateRect(window, nullptr, FALSE);
  return result;
}
BOOL CALLBACK attach(HWND window, LPARAM) {
  SetWindowSubclass(window, paintProc, subclassId, 0);
  if (isClass(window, L"Edit") || isClass(window, WC_LISTVIEWW)) {
    SetWindowTheme(window, L"", L"");
    if (isClass(window, WC_LISTVIEWW)) {
      ListView_SetBkColor(window, color(inset, COLOR_WINDOW));
      ListView_SetTextBkColor(window, color(inset, COLOR_WINDOW));
      ListView_SetTextColor(window, color(text, COLOR_WINDOWTEXT));
    }
  }
  return TRUE;
}
BOOL CALLBACK detach(HWND window, LPARAM) {
  RemovePropW(window, L"IrisHover");
  RemoveWindowSubclass(window, paintProc, subclassId);
  return TRUE;
}
void move(HWND control, int x, int y, int width, int height) {
  if (!control) return;
  RECT rect{x, y, x + width, y + height};
  if (MapDialogRect(GetParent(control), &rect))
    MoveWindow(control, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, TRUE);
}
void layout(const wchar_t* page) {
  // Keep the primary action at the far right without changing tab order or IDs.
  HWND next = GetDlgItem(root, IDOK), cancel = GetDlgItem(root, IDCANCEL);
  RECT nextRect{}, cancelRect{};
  if (next && cancel && GetWindowRect(next, &nextRect) && GetWindowRect(cancel, &cancelRect) &&
      nextRect.left < cancelRect.left) {
    MapWindowPoints(HWND_DESKTOP, root, reinterpret_cast<POINT*>(&nextRect), 2);
    MapWindowPoints(HWND_DESKTOP, root, reinterpret_cast<POINT*>(&cancelRect), 2);
    SetWindowPos(next, nullptr, cancelRect.left, nextRect.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    SetWindowPos(cancel, nullptr, nextRect.left, cancelRect.top, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
  }
  if (HWND heading = GetDlgItem(root, 1037)) {
    SendMessageW(heading, WM_SETFONT, reinterpret_cast<WPARAM>(headingFont), TRUE);
    move(heading, 16, 9, 290, 23);
    move(GetDlgItem(root, 1038), 16, 33, 290, 16);
  }
  ShowWindow(GetDlgItem(root, 1039), SW_HIDE); // Repeated header brand bitmap.
  ShowWindow(GetDlgItem(root, 1035), SW_HIDE);
  HWND panel = FindWindowExW(root, nullptr, L"#32770", nullptr);
  if (!panel) return;
  if (lstrcmpW(page, L"directory") == 0) {
    move(GetDlgItem(panel, 1020), 0, 0, 298, 14);
    move(GetDlgItem(panel, 1019), 0, 20, 245, 19);
    move(GetDlgItem(panel, 1001), 252, 20, 46, 19);
    move(GetDlgItem(panel, 1023), 0, 48, 298, 12);
    move(GetDlgItem(panel, 1024), 0, 62, 298, 12);
    move(GetDlgItem(panel, 1006), 0, 100, 298, 44);
  } else if (lstrcmpW(page, L"progress") == 0) {
    move(GetDlgItem(panel, 1006), 0, 8, 298, 16);
    move(GetDlgItem(panel, 1004), 0, 30, 298, 5);
    move(GetDlgItem(panel, 1027), 0, 48, 100, 18);
    move(GetDlgItem(panel, 1016), 0, 74, 298, 90);
  } else if (lstrcmpW(page, L"uninstall") == 0) {
    ShowWindow(GetDlgItem(panel, 1006), SW_HIDE);
    move(GetDlgItem(panel, 1029), 0, 8, 298, 16);
    move(GetDlgItem(panel, 1000), 0, 30, 298, 19);
  }
}
UINT_PTR __cdecl release(int message) {
  if (message == 0 || message == 1) {
    if (IsWindow(root)) { EnumChildWindows(root, detach, 0); detach(root, 0); }
    if (backgroundBrush) DeleteObject(backgroundBrush);
    if (insetBrush) DeleteObject(insetBrush);
    // The heading retains this font until the window dies. Restore its original font first.
    if (IsWindow(root)) SendMessageW(GetDlgItem(root, 1037), WM_SETFONT, SendMessageW(root, WM_GETFONT, 0, 0), FALSE);
    if (headingFont) DeleteObject(headingFont);
    backgroundBrush = insetBrush = nullptr;
    headingFont = nullptr;
    root = nullptr;
  }
  return 0;
}
} // namespace

extern "C" __declspec(dllexport) void __cdecl Apply(HWND parent, int, wchar_t*, Stack** stack, Extra* extra) {
  wchar_t page[32]{};
  if (stack && *stack) {
    Stack* item = *stack;
    lstrcpynW(page, item->text, 32);
    *stack = item->next;
    GlobalFree(item);
  }
  if (!parent || !extra || !extra->registerCallback || extra->registerCallback(module, release) < 0) return;
  root = parent;
  if (!backgroundBrush) {
    HIGHCONTRASTW contrast{sizeof(contrast), 0, nullptr};
    highContrast = SystemParametersInfoW(SPI_GETHIGHCONTRAST, sizeof(contrast), &contrast, 0) && (contrast.dwFlags & HCF_HIGHCONTRASTON);
    backgroundBrush = CreateSolidBrush(color(surface, COLOR_BTNFACE));
    insetBrush = CreateSolidBrush(color(inset, COLOR_WINDOW));
    headingFont = CreateFontW(-scale(root, 20), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
      DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Microsoft YaHei UI");
  }
  if (!highContrast) {
    BOOL dark = TRUE;
    DwmSetWindowAttribute(root, 20, &dark, sizeof(dark));
    COLORREF caption = RGB(18, 18, 20), title = secondary;
    DwmSetWindowAttribute(root, 35, &caption, sizeof(caption));
    DwmSetWindowAttribute(root, 36, &title, sizeof(title));
  }
  attach(root, 0);
  EnumChildWindows(root, attach, 0);
  layout(page);
  RedrawWindow(root, nullptr, nullptr, RDW_INVALIDATE | RDW_ALLCHILDREN);
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
  if (reason == DLL_PROCESS_ATTACH) { module = instance; DisableThreadLibraryCalls(instance); }
  return TRUE;
}
