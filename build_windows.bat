@echo off
rem Builds Backwoods Dash Designer for Windows. Double-click this file, or run it from a Command Prompt.
setlocal
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo.
  echo Rust is not installed yet. One time setup:
  echo   1. Install the Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/
  echo      ^(tick "Desktop development with C++"^)
  echo   2. Install Rust from https://rustup.rs  ^(the default options are fine^)
  echo   3. Close this window and run build_windows.bat again.
  echo.
  pause
  exit /b 1
)
echo Building. The first build downloads libraries and takes a few minutes...
cargo build --release
if errorlevel 1 (
  echo.
  echo The build failed. Please send a screenshot of the messages above.
  pause
  exit /b 1
)
if not exist dist mkdir dist
copy /y target\release\bw-dash-designer.exe "dist\Backwoods Dash Designer.exe" >nul
echo.
echo Done. The program is: %CD%\dist\Backwoods Dash Designer.exe
echo Projects and settings are stored in %USERPROFILE%\BackwoodsDashDesigner
echo.
pause
