#!/bin/bash
# echo.sh - Simple echo script for xgent CLI execution examples.
# Reads input from args or stdin and outputs a JSON result.
# Zero dependencies beyond bash.

if [ -n "$1" ]; then
  INPUT="$1"
else
  INPUT=$(cat)
fi

# Output a JSON result
echo "{\"output\": \"processed: ${INPUT}\", \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}"
