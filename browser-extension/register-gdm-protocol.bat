@echo off
echo ============================================================
echo   ZR Downloader - Windows Protocol Registration
echo ============================================================
echo.

set "APP_EXE=%LOCALAPPDATA%\ZR Downloader\zr-downloader.exe"

if not exist "%APP_EXE%" (
    if exist "%LOCALAPPDATA%\Goat Download Manager\gdm.exe" set "APP_EXE=%LOCALAPPDATA%\Goat Download Manager\gdm.exe"
    if exist "%ProgramFiles%\ZR Downloader\zr-downloader.exe" set "APP_EXE=%ProgramFiles%\ZR Downloader\zr-downloader.exe"
    if exist "%ProgramFiles%\Goat Download Manager\gdm.exe" set "APP_EXE=%ProgramFiles%\Goat Download Manager\gdm.exe"
    if exist "%ProgramFiles(x86)%\ZR Downloader\zr-downloader.exe" set "APP_EXE=%ProgramFiles(x86)%\ZR Downloader\zr-downloader.exe"
)

if exist "%APP_EXE%" (
    reg add "HKCU\Software\Classes\zr" /ve /d "URL:ZR Downloader Protocol" /f >nul
    reg add "HKCU\Software\Classes\zr" /v "URL Protocol" /d "" /f >nul
    reg add "HKCU\Software\Classes\zr\shell\open\command" /ve /d "\"%APP_EXE%\" \"%%1\"" /f >nul

    reg add "HKCU\Software\Classes\gdm" /ve /d "URL:ZR Downloader Protocol" /f >nul
    reg add "HKCU\Software\Classes\gdm" /v "URL Protocol" /d "" /f >nul
    reg add "HKCU\Software\Classes\gdm\shell\open\command" /ve /d "\"%APP_EXE%\" \"%%1\"" /f >nul

    echo [SUCCESS] zr:// and gdm:// protocols registered successfully!
    echo Target executable: "%APP_EXE%"
) else (
    echo [INFO] ZR Downloader executable will automatically register protocols on first launch.
)

echo.
pause
