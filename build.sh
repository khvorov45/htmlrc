ODIN_FLAGS_COMMON="-out:out/htmlrc -default-to-nil-allocator -no-crt -verbose-errors"
ODIN_FLAGS_DEBUG_OR_RELEASE="-debug"
if [ "$1" == "release" ]
  then
    ODIN_FLAGS_DEBUG_OR_RELEASE="-o:speed"
fi

odin build htmlrc $ODIN_FLAGS_COMMON $ODIN_FLAGS_DEBUG_OR_RELEASE

echo done
