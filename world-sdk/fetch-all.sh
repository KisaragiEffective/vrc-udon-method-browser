#!/bin/bash

official_repository=https://github.com/vrchat/packages/
raw_tags="$(gh release -R "$official_repository" list --json tagName --limit 100 | jq -r '.[].tagName')"

echo "$raw_tags" | \
  xargs -I {} -P 4 gh release -R "$official_repository" download {} -O ./world-sdk/trees/{}/src.zip -p 'com.vrchat.worlds-*.zip'
echo "$raw_tags" | xargs -I {} unzip ./world-sdk/trees/{}/src.zip -d ./world-sdk/trees/{}/src
