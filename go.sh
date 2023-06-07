#!/bin/sh

# Run a command but map the error code in $1 to the success code.

OK="$1"
shift

echo "Running $@"

$@
STATUS=$?

if [ "$STATUS" -eq "$OK" ] || [ "$STATUS" -eq "0" ]; then
  exit 0
else
  echo "Failure: $STATUS"
  exit 1
fi
