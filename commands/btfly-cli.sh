#!/usr/bin/env bash

case "$1" in
health)
    curl -X GET localhost:8080/health
    ;;
getbalance)
    curl -X GET localhost:8080/balance
    ;;
getbalancebyaddress)
    curl -X GET localhost:8080/balance/$2
    ;;
getchain)
    curl -X GET localhost:8080/chain
    ;;
sendtransaction)
    curl -X POST -H "Content-Type: application/json" -d "{\"recipient\":\"$2\", \"send_amount\": $3, \"fee\": $4}" localhost:8080/tx
    ;;
addpeer)
    curl -X POST -H "Content-Type: application/json" -d "{\"ip\":\"$2\"}" localhost:8080/peer
    ;;
*)
    echo "No matching argument found: $1"
    ;;
esac
