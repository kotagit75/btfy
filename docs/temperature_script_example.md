# Temperature Script Examples
## Example 1 - Use API
### Open-Meteo API
```bash
#!/bin/bash

temperature=$(curl "https://api.open-meteo.com/v1/forecast?latitude=$1&longitude=$2&current=temperature_2m" | jq .current.temperature_2m)
echo -n $temperature
exit 0
```
### OpenWeatherMap API
```bash
#!/bin/bash

API_KEY=""

temperature=$(curl "https://api.openweathermap.org/data/2.5/weather?lat=$1&lon=$2&appid=$API_KEY" | jq .main.temp)
echo -n $temperature
exit 0
```
## Example 2 - Use sensors
