#!/bin/bash
#
# Runs all .v files in TEST_DIR back-to-back
#
# Use the -s flag to skip specific files.
# Run './runall.sh -s abs.v add.v' to skip 'abs.v' and 'add.v'
#
# You may need to run 'sudo chmod +x runall.sh' if perms are denied
##


TEST_DIR="../marz"
SKIP_LIST=()

# Parse -s flag and store following words as skip list
while [[ "$1" != "" ]]; do
    case $1 in
        -s)
            shift
            while [[ "$1" != "" && "$1" != -* ]]; do
                SKIP_LIST+=("$1")
                shift
            done
            ;;
        *)
            shift
            ;;
    esac
done

# Function to check if a file is in the skip list
is_skipped() {
    local filename=$(basename "$1")
    for skip in "${SKIP_LIST[@]}"; do
        if [[ "$filename" == "$skip" ]]; then
            return 0
        fi
    done
    return 1
}

# Run each .v file unless it's in the skip list
for file in "$TEST_DIR"/*.v; do
    if is_skipped "$file"; then
        continue
    fi

    echo "🟢  Running test: $file"
    echo "------------------------"
    echo ""

    cargo run --quiet "$file"

done

echo "Finished running all .v tests."

