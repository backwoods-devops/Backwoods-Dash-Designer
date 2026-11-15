#!/bin/bash
# Builds the Windows program (a single .exe) on a Mac. Nothing needs to run on Windows to make it.
# One time: Homebrew + Rust must be installed (you already have Rust). The script installs the rest.
set -e
cd "$(dirname "$0")"
TARGET=x86_64-pc-windows-gnu
if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  echo "Installing the Windows linker (mingw-w64) with Homebrew..."
  brew install mingw-w64
fi
rustup target add $TARGET
mkdir -p .cargo
if ! grep -q "$TARGET" .cargo/config.toml 2>/dev/null; then
  printf '[target.%s]\nlinker = "x86_64-w64-mingw32-gcc"\n' "$TARGET" >> .cargo/config.toml
fi
# static: the .exe should not need any extra DLLs next to it
RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-static" cargo build --release --target $TARGET
mkdir -p dist
cp "target/$TARGET/release/bw-dash-designer.exe" "dist/Backwoods Dash Designer.exe"
echo
echo "DLLs the .exe asks Windows for (all should be built-in Windows ones):"
x86_64-w64-mingw32-objdump -p "dist/Backwoods Dash Designer.exe" | grep "DLL Name" | sort -u || true
echo
ls -lh "dist/Backwoods Dash Designer.exe"
echo "Send that file. It is an x64 program: it runs on normal Windows PCs and, emulated, in a Windows ARM VM."
