@echo off
REM Compila ClipVault a un .exe standalone (no requiere Python en la PC destino).
REM Ejecutar desde la raiz del proyecto, con el venv activado.

echo === Limpiando compilaciones anteriores ===
if exist dist rmdir /s /q dist
if exist build rmdir /s /q build

pip install pyinstaller

python -m PyInstaller --noconfirm --windowed --name ClipVault ^
  --icon=static\icon.ico ^
  --add-data "static;static" ^
  --add-data "templates;templates" ^
  --hidden-import=engineio.async_drivers.threading ^
  --hidden-import=win32timezone ^
  main.py

echo.
echo === Build terminado en dist\ClipVault ===
echo Copiando yt-dlp.exe y ffmpeg junto al ejecutable...
copy yt-dlp.exe dist\ClipVault\yt-dlp.exe
copy ffmpeg.exe dist\ClipVault\ffmpeg.exe
copy ffprobe.exe dist\ClipVault\ffprobe.exe

echo.
echo === Verificando que no se filtren datos personales ===
if exist "dist\ClipVault\settings.json" (
    echo ADVERTENCIA: settings.json encontrado en dist, borrando...
    del /f /q "dist\ClipVault\settings.json"
)
if exist "dist\ClipVault\profiles.json" (
    echo ADVERTENCIA: profiles.json encontrado en dist, borrando...
    del /f /q "dist\ClipVault\profiles.json"
)
if exist "dist\ClipVault\history.json" (
    echo ADVERTENCIA: history.json encontrado en dist, borrando...
    del /f /q "dist\ClipVault\history.json"
)
if exist "dist\ClipVault\downloads" (
    echo ADVERTENCIA: carpeta downloads encontrada en dist, borrando...
    rmdir /s /q "dist\ClipVault\downloads"
)
if exist "dist\ClipVault\logs" (
    echo ADVERTENCIA: carpeta logs encontrada en dist, borrando...
    rmdir /s /q "dist\ClipVault\logs"
)

echo.
echo === Listo. dist\ClipVault esta limpio y listo para Inno Setup ===
pause
