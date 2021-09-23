@echo off

set ODIN_FLAGS_COMMON=-out:build/htmlrc.exe -verbose-errors
set ODIN_FLAGS_DEBUG_OR_RELEASE=-debug
if "%1"=="release" set ODIN_FLAGS_DEBUG_OR_RELEASE=-o:speed

odin build code/htmlrc.odin %ODIN_FLAGS_COMMON% %ODIN_FLAGS_DEBUG_OR_RELEASE%

echo done
