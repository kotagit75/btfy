#!/usr/bin/env python3
import json
import sys

def main():
    for _ in sys.stdin:
        print(json.dumps({"temperature": int(round(10))}), flush=True)


if __name__ == "__main__":
    main()
