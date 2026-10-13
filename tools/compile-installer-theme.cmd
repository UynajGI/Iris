@echo off
rem SPDX-License-Identifier: GPL-3.0-or-later
call "%~1" x86 >nul
if errorlevel 1 exit /b %errorlevel%
cl.exe /nologo /std:c++17 /EHsc /W4 /WX /MT /LD /DUNICODE /D_UNICODE /Fo"%~3\theme.obj" /Fe"%~3\IrisTheme.dll" "%~2" /link /DEF:"%~4" user32.lib gdi32.lib comctl32.lib dwmapi.lib uxtheme.lib
if errorlevel 1 exit /b %errorlevel%
cl.exe /nologo /std:c++17 /EHsc /W4 /WX /MT /DUNICODE /D_UNICODE /Fo"%~3\test.obj" /Fe"%~3\theme-test.exe" "%~dp2test.cpp" /link /MANIFEST:EMBED /MANIFESTINPUT:"%~dp2test.manifest" user32.lib gdi32.lib comctl32.lib dwmapi.lib uxtheme.lib
if errorlevel 1 exit /b %errorlevel%
"%~3\theme-test.exe"
exit /b %errorlevel%
