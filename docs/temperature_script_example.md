# Temperature Script Examples
## Example 1 - Use API

```python
#!/usr/bin/env python3
import json
import sys
import urllib.request
from datetime import datetime, timezone
from urllib.parse import urlencode

OPEN_METEO_BASE = "https://api.open-meteo.com/v1/forecast"


def fetch_open_meteo(lat: str, lon: str) -> dict:
    params = {
        "latitude": lat,
        "longitude": lon,
        "past_days": "10",
        "hourly": "temperature_2m",
    }
    url = OPEN_METEO_BASE + "?" + urlencode(params)
    req = urllib.request.Request(
        url, headers={"User-Agent": "btfy-temperature-server/1.0"}
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        return json.loads(resp.read().decode("utf-8"))


def time_to_epoch_seconds(t: str) -> int:
    dt = datetime.strptime(t, "%Y-%m-%dT%H:%M").replace(tzinfo=timezone.utc)
    return int(dt.timestamp())


def select_temperature(data: dict, ts: int) -> float:
    hourly = data.get("hourly") or {}
    times = hourly.get("time") or []
    temps = hourly.get("temperature_2m") or []
    if not times or not temps or len(times) != len(temps):
        raise ValueError("invalid open-meteo response")

    if ts == 0:
        return float(temps[-1])

    best_i = 0
    best_diff = None
    for i, t in enumerate(times):
        te = time_to_epoch_seconds(t)
        diff = abs(te - ts)
        if best_diff is None or diff < best_diff:
            best_diff = diff
            best_i = i

    return float(temps[best_i])


def main():
    for line in sys.stdin:
        req = line.strip().split()
        lat = float(req[0])
        lon = float(req[1])
        ts = int(req[2])
        data = fetch_open_meteo(str(lat), str(lon))
        temp = select_temperature(data, ts)
        print(json.dumps({"temperature": int(round(temp * 10))}), flush=True)


if __name__ == "__main__":
    main()
```

## Example 2 - Use sensors
