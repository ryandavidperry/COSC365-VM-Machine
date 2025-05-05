# Runs all .v files in TEST_DIR back to back
#
# You may need to run 'sudo chmod +x runall.sh' if perms are denied

TEST_DIR="../marz"

# Loop through each .v file
for file in "$TEST_DIR"/*.v; do
    echo "------------------------------------"
    echo "🟢  Running: $file"
    echo "------------------------------------"

    cargo run --quiet "$file"

    echo ""
done

echo "Finished running all .v tests."

