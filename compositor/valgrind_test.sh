#!/bin/bash
rm pipelines/*
RUST_LOG=debug 
rm ../target/debug/deps/compositor-*
cargo test --no-run
valgrind  --tool=massif ../target/debug/deps/compositor-???????????????? $1
