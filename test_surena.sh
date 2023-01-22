#!/bin/bash

# This script performs a small integration test on the game using surena.

set -eEuo pipefail

if [[ $# -ne 2 ]]; then
	echo "Usage: $0 <PATH_TO_SURENA> <PATH_TO_LIB>" >&2
	exit 2
fi

INPUT="\
/load_plugin $2
/create std O \"4x3@3\"
/pov 1
0
/pov 2
1
/pov 1
invalid
1
/pov 2
10
2
/pov 1
2
/pov 2
3
/print
/destroy
/exit"

EXPECTED="\
| | | | |
| |X|X| |
|X|O|O|O|
 0 1 2 3"

OUTPUT="$(echo "$INPUT" | $1 repl)"
if [[ "$OUTPUT" != *"$EXPECTED"* ]]; then
	echo "$OUTPUT"
	echo "Got unexpected output from surena!" >&2
	exit 1
fi
