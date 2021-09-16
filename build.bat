@echo off

set ODIN_FLAGS_COMMON=-out:out/htmlrc.exe -default-to-nil-allocator -verbose-errors
set ODIN_FLAGS_DEBUG_OR_RELEASE=-debug
if "%1"=="release" set ODIN_FLAGS_DEBUG_OR_RELEASE=-o:speed

odin build win32.odin %ODIN_FLAGS_COMMON% %ODIN_FLAGS_DEBUG_OR_RELEASE%

echo done
exit 0
