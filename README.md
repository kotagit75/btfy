<div align="center">
    <img src="assets/logo.svg" height=200>
    <h1>Dawn</h1>
    <h2>Energy-Efficient Cryptocurrency By "Proof of Weather"</h2>
</div>

Dawn is a cryptocurrency that relies on the randomness of the weather and cryptographic proofs as its foundation.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![GitHub Actions Results](https://github.com/kotagit75/Dawn/actions/workflows/test.yaml/badge.svg)

> [!NOTE]
> Dawn is currently in active development. The API and features may change without notice.

> [!NOTE]
> A Japanese article explaining Dawn can be found [here](https://zenn.dev/yuzu_mikan/articles/7e5df1520f183a).

## :sparkles: Features
- ⛅ Consensus by Weather - Weather data enables rapid consensus building
- ⚡ Highly energy-efficient - Because VDF is used instead of Proof of Work, it is more energy-efficient

## :dart: How does it work?
Weather is a source of information where, regardless of who observes it, relatively consistent readings are obtained at the same time; however, it is impossible to predict its changes with absolute accuracy. By incorporating this characteristic of weather into the consensus mechanism of a decentralized system, we can create a currency that does not require proof-of-work.

Here, data that is difficult to predict is referred to as a "beacon." Dawn uses a hash chain composed of linked blocks. In addition to transactions, each block contains the beacon and the results of VDF calculations. Because the value of the beacon is difficult to predict, it is challenging to generate future blocks. Furthermore, the hash chain is employed to prevent the generation of blocks using past beacon values.
## :rocket: Quick Start
```bash
git clone https://github.com/kotagit75/Dawn.git
cd Dawn
cargo build --release
mkdir beacon
cp example/beacon/open-meteo-api beacon/temperature
chmod +x beacon/temperature
./target/release/dawn
```

[Detailed Installation Instructions](docs/installation.md)

### Usage
```bash
# run
./target/release/dawn

# get balance
curl localhost:8080/balance
curl localhost:8080/balance/[address]

# get chain
curl localhost:8080/chain

# send transaction
curl -X POST -H "Content-Type: application/json" -d "{'recipient':'[address]', 'amount': [amount]}" localhost:8080/tx

# add peer
curl -X POST -H "Content-Type: application/json" -d '{"ip":"[IP Addr]"}' localhost:8080/peer
```

## :globe_with_meridians: Environment variables
| Name | Description | Default |
| :--- | :--- | :--- |
| `API_PORT` | API server port number | `8080` |
| `CORS_ALLOW_PORT` | CORS allow port number | `3000` |
## 📍 Locations which is collected temperature data
Dawn gets temperature data from multiple locations. They are currently placed in Japan. The locations are as follows:

|Name|Latitude|Longitude|
|:-:|:-:|:-:|
|Wakkanai Airport|45.3995654|141.7974528|
|Asahikawa Airport|43.67147493|142.446865|
|Kushiro Airport|43.04503509|144.1962358|
|Obihiro Airport|42.73121032|143.2177867|
|Sapporo Okadama Airport|43.11577495|141.3802179|
|New Chitose Airport|42.77899571|141.6860269|
|Hakodate Airport|41.7754762|140.8161369|
|Aomori Airport|40.73545867|140.6902087|
|Akita Airport|39.61432074|140.2176736|
|Hanamaki Airport|39.42148821|141.1384845|
|Sendai Airport|38.13993289|140.9170924|
|Yamagata Airport|38.41209636|140.3703334|
|Fukushima Airport|37.2284081|140.4282886|
|Niigata Airport|37.95505405|139.1114496|
|Matsumoto Airport|36.16462046|137.9264258|
|Narita International Airport|35.77073692|140.3848188|
|Tokyo International Airport|35.548171|139.7791314|
|Shizuoka Airport|34.79653615|138.1853326|
|Chubu Centrair International Airport|34.85720324|136.8101604|
|Osaka Itami Airport|34.78606811|135.4381271|
|Kansai International Airport|34.43197865|135.2367959|
|Kobe Airport|34.63507139|135.2267252|
|Takamatsu Airport|34.21484194|134.0146539|
|Kochi Airport|33.5476357|133.6739953|
|Hiroshima Airport|34.43731367|132.9207516|
|Matsuyama Airport|33.8277126|132.7003022|
|Yamaguchi Ube Airport|33.93127097|131.2786026|
|Fukuoka Airport|33.58561376|130.4500511|
|Nagasaki Airport|32.91489785|129.9170527|
|Oita Airport|33.47958263|131.7362115|
|Kumamoto Airport|32.83497974|130.8588813|
|Kagoshima Airport|31.80072839|130.7202485|
|Naha Airport|26.19990739|127.6467932|


[View geojson](src/beacon/target.geojson)

```geojson
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "properties": {
        "Name": "Wakkanai Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [141.7974528, 45.3995654]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Asahikawa Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [142.446865, 43.67147493]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kushiro Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [144.1962358, 43.04503509]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Obihiro Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [143.2177867, 42.73121032]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Sapporo Okadama Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [141.3802179, 43.11577495]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "New Chitose Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [141.6860269, 42.77899571]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Hakodate Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.8161369, 41.7754762]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Aomori Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.6902087, 40.73545867]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Akita Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.2176736, 39.61432074]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Hanamaki Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [141.1384845, 39.42148821]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Sendai Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.9170924, 38.13993289]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Yamagata Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.3703334, 38.41209636]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Fukushima Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.4282886, 37.2284081]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Niigata Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [139.1114496, 37.95505405]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Matsumoto Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [137.9264258, 36.16462046]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Narita International Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [140.3848188, 35.77073692]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Tokyo International Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [139.7791314, 35.548171]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Shizuoka Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [138.1853326, 34.79653615]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Chubu Centrair International Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [136.8101604, 34.85720324]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Osaka Itami Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [135.4381271, 34.78606811]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kansai International Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [135.2367959, 34.43197865]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kobe Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [135.2267252, 34.63507139]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Takamatsu Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [134.0146539, 34.21484194]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kochi Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [133.6739953, 33.5476357]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Hiroshima Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [132.9207516, 34.43731367]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Matsuyama Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [132.7003022, 33.8277126]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Yamaguchi Ube Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [131.2786026, 33.93127097]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Fukuoka Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [130.4500511, 33.58561376]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Nagasaki Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [129.9170527, 32.91489785]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Oita Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [131.7362115, 33.47958263]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kumamoto Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [130.8588813, 32.83497974]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Kagoshima Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [130.7202485, 31.80072839]
      }
    },
    {
      "type": "Feature",
      "properties": {
        "Name": "Naha Airport"
      },
      "geometry": {
        "type": "Point",
        "coordinates": [127.6467932, 26.19990739]
      }
    }
  ]
}

```
