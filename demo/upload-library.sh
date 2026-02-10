#!/bin/bash
set -euo pipefail

# Generate all-patterns.yml listing all .yml pattern files
(cd demo/library && ls *.yml | grep -v all-patterns.yml | sed 's/\.yml$//' | sed 's/^/- /' > all-patterns.yml)

# Upload all .yml files to S3
AWS_PROFILE=zik-laurent aws s3 sync demo/library/ s3://zik-laurent/drum-patterns/ \
  --exclude "*" \
  --include "*.yml"
