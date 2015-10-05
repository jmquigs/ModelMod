@echo off
cls
".paket\paket.exe" install
"packages\FAKE\tools\Fake.exe" build.fsx
pause