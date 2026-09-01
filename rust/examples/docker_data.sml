@version v4

base: "ubuntu:22.04"
maintainer: "sakeen <***REMOVED***>"
workdir: "/app"
ports: [ "8080" "9090" ]
deps: [ "curl" "git" "build-essential" ]
cmd: "[\"python3\", \"app.py\"]"
