@version v4

base: "ubuntu:22.04"
maintainer: "dev"
workdir: "/app"
ports: [ "8080" "9090" ]
deps: [ "curl" "git" "build-essential" ]
cmd: "[\"python3\", \"app.py\"]"
