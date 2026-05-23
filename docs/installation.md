### Installation
```bash
# Clone the repository (or Download ZIP)
git clone https://github.com/kotagit75/Dawn.git

# Navigate to the project directory
cd Dawn

# build
cargo build --release
```
### Create a script to retrieve the temperature
```bash
mkdir beacon
```
Create a shell script named `beacon/temperature`. This script retrieves the latitude ,longitude and timestamp and returns the temperature at that location as `stdout`. It doesn't matter how you implement it. Here is an example. Note that this API does not actually exist.
```bash
#!/bin/bash

if [ $3 -eq 0 ]; then
    temperature=$(curl "https://example.com/api?latitude=$1&longitude=$2")
else
    date --date @$3 +"%Y-%m-%d %H:%M:%S"
    temperature=$(curl "https://example.com/api?latitude=$1&longitude=$2&timestamp=$3")
fi
echo -n $temperature
exit 0
```
Even without using an API, it is possible to conduct observations by placing sensors on-site, for example.

[Learn more examples](./temperature_script_example.md)
