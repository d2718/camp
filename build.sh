#!/usr/bin/bash

set -xeuo pipefail

TARGET=x86_64-unknown-linux-musl
BINARY=target/$TARGET/release/camp
ARTIFACT=us-east1-docker.pkg.dev/camp-357714/camp-repo/camp

cargo build --target $TARGET --release
strip $BINARY

docker build -f main.Dockerfile -t camp -t $ARTIFACT .
docker push $ARTIFACT

