#!/usr/bin/env bash

set -euo pipefail

while read -r lat lon ts; do
    sleep 0.1
    echo "{\"temperature\": 1}"
done
