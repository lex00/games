#!/bin/sh
# The twelve paintings left, one call each, 5 credits a call. Stops at the first failure.
cd "$(dirname "$0")"
for k in 00-desert 02-waterfall 03-cloud 04-coach 05-cattle 06-buffalo 07-homestead 08-sunset 09-camp 10-storm 11-lake 12-gate; do
  GEN_QUALITY=medium python3 gen.py paint16-$k gpt_image_2 1920:1280 5 prompts/paint16-$k.txt counter=refs/bar-style.png canyon=refs/canyon-ref.png || exit 1
done
