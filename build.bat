@echo off
SET TARGET="FullBuild"

IF NOT [%1]==[] (set TARGET="%1")

cls
".paket\paket.exe" install
"packages\FAKE\tools\Fake.exe" build.fsx "target=%TARGET%"

if "%2"=="nopause" goto exit
pause
:exit
