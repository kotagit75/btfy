#!/usr/bin/env python3
import json
import urllib.request
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlencode, urlparse

OPEN_METEO_BASE = "https://api.open-meteo.com/v1/forecast"


def fetch_open_meteo(lat: str, lon: str) -> dict:
    params = {
        "latitude": lat,
        "longitude": lon,
        "past_days": "10",
        "hourly": "temperature_2m,relative_humidity_2m,wind_speed_10m",
    }
    url = OPEN_METEO_BASE + "?" + urlencode(params)
    req = urllib.request.Request(
        url, headers={"User-Agent": "btfly-temperature-server/1.0"}
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


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        u = urlparse(self.path)
        if u.path != "/":
            self.send_response(404)
            self.end_headers()
            return

        qs = parse_qs(u.query)
        lat = (qs.get("lat") or [""])[0]
        lon = (qs.get("lon") or [""])[0]
        ts_s = (qs.get("timestamp") or ["0"])[0]

        try:
            ts = int(ts_s)
            if not lat or not lon:
                raise ValueError("missing latitude/longitude")

            data = fetch_open_meteo(lat, lon)
            temp = select_temperature(data, ts)

            body = (str(temp)).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except Exception as e:
            body = (f"error: {e}").encode("utf-8")
            self.send_response(400)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)


def main():
    host = "127.0.0.1"
    port = 8000
    httpd = ThreadingHTTPServer((host, port), Handler)
    print(f"temperature server: http://{host}:{port}/")
    httpd.serve_forever()


if __name__ == "__main__":
    main()
