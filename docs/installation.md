### Installation
```bash
# Install openssl
sudo apt -y install openssl

# Clone the repository (or Download ZIP)
git clone https://github.com/kotagit75/btfy.git

# Navigate to the project directory
cd btfy
```

### Create a script to retrieve the temperature
Create a script. This script reads latitude, longitude, and timestamp from stdin and writes the temperature to stdout as JSON. It doesn't matter how you implement it.
Even without using an API, it is possible to conduct observations by placing sensors on-site, for example.
[Learn more examples](./temperature_script_example.md)

### Run
Run btfy.
```bash
cargo run --release -- --mining --beacon-cmd python3 --beacon-cmd example/open-meteo.py
```
