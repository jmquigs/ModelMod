# run test 10 times in a loop exit if any errors
for i in {1..10}; do
    echo "Run $i"
    cargo test --release
    if [ $? -ne 0 ]; then
        echo "Test failed"
        exit 1
    fi
done
