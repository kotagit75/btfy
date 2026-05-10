<div align="center">
    <img src="assets/logo_with_name.svg" height=100>
</div>

Dawn is a decentralized currency that relies on the randomness of the weather and cryptographic proofs as its foundation.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)

> [!NOTE]
> Dawn is currently in active development. The API and features may change without notice.

## :sparkles: Features
- ⛅ Consensus by Weather - Weather data enables rapid consensus building
- ⚡ Highly energy-efficient - Because VDF is used instead of Proof of Work, it is more energy-efficient

## :dart: How does it work?
Weather is a source of information where, regardless of who observes it, relatively consistent readings are obtained at the same time; however, it is impossible to predict its changes with absolute accuracy. By incorporating this characteristic of weather into the consensus mechanism of a decentralized system, we can create a currency that does not require proof-of-work.

Here, data that is difficult to predict is referred to as a "beacon." Dawn uses a hash chain composed of linked blocks. In addition to transactions, each block contains the beacon and the results of VDF calculations. Because the value of the beacon is difficult to predict, it is challenging to generate future blocks. Furthermore, the hash chain is employed to prevent the generation of blocks using past beacon values.
## :rocket: Get started
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
Create a shell script named `beacon/temperature`.This script retrieves the latitude and longitude and returns the temperature at that location as `stdout`.It doesn't matter how you implement it. Here is an example. Note that this API does not actually exist.
```bash
#!/bin/bash
temperature=$(curl "https://example.com/api?latitude=$1&longitude=$2")
echo -n $temperature
exit 0
```
Even without using an API, it is possible to conduct observations by placing sensors on-site, for example.

[Learn more examples](docs/temperature_script_example.md)
### Usage
```bash
# run
./target/release/dawn

# get balance
curl localhost:8080/balance

# get chain
curl localhost:8080/chain

# send transaction
curl -X POST -H "Content-Type: application/json" -d "{'recipient':'{address}', 'amount': {amount}}" localhost:8080/tx

# add peer
curl -X POST -H "Content-Type: application/json" -d '{"ip":"{IP Addr}"}' localhost:8080/peer
```

## 📍 Locations which is collected temperature data
Dawn gets temperature data from multiple regions. The regions are as follows:
- Hakodate `140.7290611111111, 41.76869722222222`
- Hirosaki `140.421492, 40.61632`
- Sendai `140.8694166666667, 38.26819444444445`
- Yokohama `139.63438781464149, 35.45023396820895`
- Nagoya `136.9065583333334, 35.18145`
- Kyoto `135.76815, 35.01156388888889`
- Kobe `135.1956305555556, 34.69008055555555`
- Hiroshima `132.4553055555556, 34.38528888888889`
- Fukuoka `130.4016888888889, 33.59018333333334`
- Kagoshima `130.5571231843784, 31.596708556139077`

[View geojson](src/beacon/target.geojson)

```geojson
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [130.401689, 33.590183, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [140.729061, 41.768697, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [139.63438781464149, 35.45023396820895, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": { "type": "Point", "coordinates": [135.76815, 35.011564, 0] },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [140.869417, 38.268194, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [135.195631, 34.690081, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": { "type": "Point", "coordinates": [136.906558, 35.18145, 0] },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [132.455306, 34.385289, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [140.421492, 40.61632, 0]
      },
      "properties": {}
    },
    {
      "type": "Feature",
      "geometry": {
        "type": "Point",
        "coordinates": [130.5571231843784, 31.596708556139077, 0]
      },
      "properties": {}
    }
  ]
}
```
