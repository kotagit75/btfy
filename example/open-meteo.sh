#!/usr/bin/env bash

set -euo pipefail

OPEN_METEO_BASE="https://api.open-meteo.com/v1/forecast"
USER_AGENT="btfy-temperature-server/1.0"

fetch_open_meteo() {
    local lat="$1"
    local lon="$2"

    curl -sS -A "$USER_AGENT" -G "$OPEN_METEO_BASE" \
        --data-urlencode "latitude=$lat" \
        --data-urlencode "longitude=$lon" \
        --data-urlencode "past_days=10" \
        --data-urlencode "hourly=temperature_2m"
}

select_temperature() {
    local json="$1"
    local ts="$2"

    jq -r \
        --argjson target "$ts" '
        .hourly
        | [ range(0; (.time|length)) as $i
            | {
                temp: .temperature_2m[$i],
                diff: (
                    (
                        .time[$i]
                        + ":00Z"
                        | fromdateiso8601
                    ) - $target
                    | if . < 0 then -. else . end
                )
            }
          ]
        | min_by(.diff)
        | .temp
        ' <<<"$json"
}

while read -r lat lon ts; do
    json=$(fetch_open_meteo "$lat" "$lon")
    temp=$(select_temperature "$json" "$ts")

    temp10=$(awk -v t="$temp" 'BEGIN { printf("%d", int(t*10+0.5)) }')

    echo "{\"temperature\": $temp10}"
done
