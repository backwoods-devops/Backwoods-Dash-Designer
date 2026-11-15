Backwoods Dash Designer - Windows test build
=============================================

This is an early test version. It has been run on a Mac but not yet on Windows, so please
tell us everything that looks wrong or does not work.

WHAT YOU NEED (one time, about 15 minutes)
  1. Visual Studio Build Tools (free): https://visualstudio.microsoft.com/visual-cpp-build-tools/
     Tick "Desktop development with C++" and install.
  2. Rust (free): https://rustup.rs  - run the installer and accept the defaults.
  3. Restart the computer or open a new window so the tools are found.

BUILD AND RUN
  1. Unzip this folder anywhere.
  2. Double-click build_windows.bat. The first build takes a few minutes.
  3. When it says Done, open  dist\Backwoods Dash Designer.exe
     (Windows may warn about an unknown publisher: More info -> Run anyway. The program is not signed yet.)

WHERE YOUR WORK IS SAVED
  Projects and settings: C:\Users\<you>\BackwoodsDashDesigner
  Dash files end in .bwd

THINGS WE ESPECIALLY WANT TO KNOW ABOUT
  - File > Open / Save as: does the Windows file window appear and work?
  - Shape editor: importing a trace image, importing an STL.
  - Export STL(s): do the files appear in the exports folder? ("Show exports folder" button)
  - Anything that crashes, looks cut off, or is too small or large on your screen.
  If something goes wrong, a log is written to  C:\Users\<you>\BackwoodsDashDesigner  (errors.log if it exists): please send it.

ALREADY GOT A BUILT .EXE?
  If you were sent "Backwoods Dash Designer.exe" you do not need any of the build steps above. Just double-click it.
  Windows SmartScreen may say "Windows protected your PC": click More info, then Run anyway (the program is not signed yet).
  The window and taskbar show the BWF icon, and the program always uses its dark look whatever your Windows setting is.
  Email often blocks .exe files, even zipped: a Drive, Dropbox or similar link works.
