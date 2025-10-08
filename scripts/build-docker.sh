#!/bin/bash
set -e
IMAGE_NAME="dynip-email"
TAG="latest"
docker buildx build --platform linux/amd64 -t "$IMAGE_NAME:$TAG" .

if [ -n "$SSH_HOST" ]; then
  echo "Deploying to $SSH_HOST"
  docker save "$IMAGE_NAME:$TAG" | bzip2 | pv | ssh -o 'RemoteCommand=none' "$SSH_HOST"  'bunzip2 | docker load'
else
  echo 'Set $SSH_HOST to automatically deploy'
fi