#!/usr/bin/env python3
import json
import sys
from time import sleep

def main():
    for _ in sys.stdin:
        sleep(0.1)
        print(json.dumps({"temperature": int(round(10))}), flush=True)


if __name__ == "__main__":
    main()
