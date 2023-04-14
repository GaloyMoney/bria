#!/bin/bash

if [[ -f version/version ]]; then
  echo "VERSION=$(cat version/version)" >> repo/.env
fi

echo "COMMITHASH=$(cat repo/.git/ref)" >> repo/.env
echo "BUILDTIME=$(date -u '+%F-%T')" >> repo/.env
