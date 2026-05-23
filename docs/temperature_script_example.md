# Temperature Script Examples
## Example 1 - Use API
```bash
#!/bin/bash

if [ $3 -eq 0 ]; then
    temperature=$(curl "[API URL]?latitude=$1&longitude=$2")
else
    date --date @$3 +"%Y-%m-%d %H:%M:%S"
    temperature=$(curl "[API URL]?latitude=$1&longitude=$2&timestamp=$3")
fi
echo -n $temperature
exit 0


```
<!--### OpenWeatherMap API
```bash
#!/bin/bash

API_KEY="[your api key]"

temperature=$(curl "https://api.openweathermap.org/data/2.5/weather?lat=$1&lon=$2&appid=$API_KEY" | jq .main.temp)
echo -n $temperature
exit 0
```
-->
## Example 2 - Use sensors
