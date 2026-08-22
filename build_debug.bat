@echo off
REM Version de debug del build: SIN --windowed, para ver el traceback real
REM si el servidor Flask falla al arrancar dentro del .exe.

python -m PyInstaller --noconfirm --name ClipVault ^
  --icon=static\icon.ico ^
  --add-data "static;static" ^
  --add-data "templates;templates" ^
  --hidden-import=engineio.async_drivers.threading ^
  --hidden-import=win32timezone ^
  --hidden-import=dns ^
  --hidden-import=dns.resolver ^
  main.py

echo.
echo === Build de debug terminado en dist\ClipVault ===
copy yt-dlp.exe dist\ClipVault\yt-dlp.exe
copy ffmpeg.exe dist\ClipVault\ffmpeg.exe
copy ffprobe.exe dist\ClipVault\ffprobe.exe

pause