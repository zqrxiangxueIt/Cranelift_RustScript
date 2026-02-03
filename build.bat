@echo off
echo Building Sample App...
cd sample_app
cargo build --release
if %errorlevel% neq 0 (
    echo Build failed!
    exit /b %errorlevel%
)
echo Build successful!
echo Running Sample App...
target\release\sample_app.exe
cd ..
