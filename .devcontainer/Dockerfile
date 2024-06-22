FROM rust:1.76.0-slim-buster

RUN apt-get update \
	&& apt-get upgrade --no-install-recommends -y --force-yes \
	&& apt-get install -y --no-install-recommends \
	curl \
	build-essential \
	libssl-dev \
	openssh-client \
	pkg-config \
	cmake \
	&& apt-get clean && rm -rf /var/lib/apt/lists/*

SHELL [ "/bin/bash", "-o", "pipefail", "-c" ]
RUN rustup component add rust-analyzer rustfmt clippy
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall --no-confirm just
