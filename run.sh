#!/bin/bash
podman build -t load_balancer .
podman run \
    --rm --name lb \
    -v $PWD/load_balancer/:/usr/src/load_balancer \
    -v $PWD/tmp:/tmp \
    -p 8000:6188 \
    -p 8001:6189 \
    load_balancer 