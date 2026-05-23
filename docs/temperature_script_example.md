# Temperature Script Examples
## Example 1 - Use API

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
