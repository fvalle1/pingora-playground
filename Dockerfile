FROM docker.io/rust:1.70

WORKDIR /usr/src/load_balancer

RUN apt update && apt install cmake -y \
	&& apt-get clean

ENTRYPOINT RUST_LOG=INFO cargo run -- -c conf.yaml

