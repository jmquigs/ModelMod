@echo off
SET TARGET="Default"

IF NOT [%1]==[] (set TARGET="%1")

cls
".paket\paket.exe" install
"packages\FAKE\tools\Fake.exe" build.fsx "target=%TARGET%"
pause