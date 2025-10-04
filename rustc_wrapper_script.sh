#!/usr/bin/env bash

at_crate_name=false

for i in "$@" ; do
    if [[ "$i" == "--crate-name" ]] ; then
        at_crate_name=true
        continue
    elif [[ "$at_crate_name" == "true" ]] ; then
        if [[ "$i" == "miros" ]] ; then
            set -- "$@" -C target-feature=+crt-static -C link-arg=-nostartfiles
        fi
        break
    fi
done

exec rustc "$@"
