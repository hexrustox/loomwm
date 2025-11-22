set dotenv-load
set shell := ["bash", "-uc"]

default: up dev

up:
    podman start $CONTAINER_NAME || podman compose up -d

sh:
    podman exec -it $CONTAINER_NAME /bin/sh

dev:
    podman exec -it $CONTAINER_NAME /bin/sh -c 'nix develop'

down:
    podman stop $CONTAINER_NAME -t 0 && podman compose down

lsp:
    @podman exec -i $CONTAINER_NAME nix develop --command rust-analyzer

spellcheck-lsp:
    @podman exec -i $CONTAINER_NAME nix develop --command codebook-lsp serve

create-volume:
    podman volume create nix-dir && podman volume create dot-cache && podman volume create dot-cargo
