#!/bin/bash
set -e

docker-compose up -d

# Move into the rust project directory
(cd rust && cargo run)

docker-compose down
