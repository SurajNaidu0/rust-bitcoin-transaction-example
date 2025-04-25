Give the run.sh script execute permission
chmod +x run.sh

Run the project
./run.sh

This script will:

Start required Docker services with docker-compose

Run the Rust app located in the rust/ directory

Automatically stop the Docker containers after the Rust app finishes
