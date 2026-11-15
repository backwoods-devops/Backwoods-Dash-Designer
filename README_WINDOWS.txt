Backwoods Dash Designer - Windows notes
=======================================

DOWNLOAD (no build needed)
  Download Backwoods-Dash-Designer-windows.zip from the Releases page, unzip it, and run
  "Backwoods Dash Designer.exe".
  The program is not code-signed. If Windows SmartScreen shows "Windows protected your PC",
  click More info, then Run anyway.

BUILD FROM SOURCE
  Requirements (one time):
    1. Visual Studio Build Tools (free), with the "Desktop development with C++" workload:
       https://visualstudio.microsoft.com/visual-cpp-build-tools/
    2. Rust (stable): https://rustup.rs
    3. Restart the computer or open a new window so the tools are found.
  Then:
    1. Unzip or clone the source anywhere.
    2. Run build_windows.bat. The first build takes a few minutes.
    3. When it reports Done, run  dist\Backwoods Dash Designer.exe

WHERE YOUR WORK IS SAVED
  Projects and settings: %USERPROFILE%\BackwoodsDashDesigner
  Dash files end in .bwd
  Errors, if any, are logged to errors.log in the same folder.

TESTING STATUS
  The Windows build is produced automatically by GitHub Actions. It has had less hands-on
  testing than the Mac version. Reports are welcome, especially on:
    - File > Open / Save as (the Windows file dialog)
    - Importing a trace image or an STL in the shape editor
    - Export STL(s) (files appear in the exports folder; "Show exports folder" opens it)
    - Window or text sizing on high-DPI or multi-monitor setups
  Please open an issue on GitHub and attach errors.log if it exists.
