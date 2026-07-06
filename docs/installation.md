### Installation
```bash
# Install openssl
sudo apt -y install openssl

# Clone the repository (or Download ZIP)
git clone https://github.com/kotagit75/btfy.git

# Navigate to the project directory
cd btfy

# Make the btfy-cli script executable
chmod +x commands/btfy-cli
```

### Create a script to retrieve the temperature
Create a script. This script retrieves the latitude ,longitude and timestamp and returns the temperature at that location as server. It doesn't matter how you implement it.
Even without using an API, it is possible to conduct observations by placing sensors on-site, for example.
[Learn more examples](./temperature_script_example.md)

### Run

First, let’s run the script for retrieving temperature data that we created in the previous chapter.
```bash
# Example
python3 examples/open-meteo.py
```
Next, run btfy.
```bash
cargo run --release -- --mining
```
You can run these two commands together using the following command:
```bash
chmod +x ./commands/run.sh
./commands/run.sh example/open-meteo.py
```
