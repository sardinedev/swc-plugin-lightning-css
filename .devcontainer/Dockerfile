# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.231.0/containers/rust/.devcontainer/base.Dockerfile

# [Choice] Debian OS version (use bullseye on local arm64/Apple Silicon): buster, bullseye
ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && curl -sL https://deb.nodesource.com/setup_16.x | sudo bash - \
    && sudo apt -y install nodejs
