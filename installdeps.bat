".paket\paket.bootstrapper.exe"
".paket\paket.exe" install

if "%1"=="nopause" goto exit


pause
:exit
