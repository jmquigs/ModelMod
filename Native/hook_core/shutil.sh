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
