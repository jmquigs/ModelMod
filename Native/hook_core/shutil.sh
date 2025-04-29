#!/bin/bash

function check_tc {
    local REQ=$1
    ATC=$(rustup show active-toolchain | grep $REQ)

    if [ "$ATC" == "" ]; then
        echo "active toolchain does not contain required string $REQ"
        echo "switch to a toolchain that does (using 'rustup default tcname')"
        echo "use 'rustup show' to list toolchains"
        exit 1
    fi

    echo "==> USING TOOLCHAIN: $ATC"
}

# set MMPATH to source script dir before calling this
function find_mm {
    MMPATH=$(realpath "$MMPATH")
    # Walk upward until we find "ModelMod"
    while [ "$MMPATH" != "/" ]; do
        if [ -d "$MMPATH/ModelMod" ]; then
            # Found it: set MMPATH to the ModelMod directory
            MMPATH="$MMPATH/ModelMod"
            echo "Found ModelMod at: $MMPATH"
            return
        fi
        # Move one directory up
        MMPATH=$(dirname "$MMPATH")
    done

    # If we exit the loop, we didn't find it
    echo "Error: ModelMod directory not found."
    exit 1
}