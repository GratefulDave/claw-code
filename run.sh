#!/usr/bin/env bash
# Local runner for claw-code — env vars scoped to this single process only.
set -euo pipefail
cd "$(dirname "$0")/rust"

OPENAI_API_KEY="ae5c1598a2d4454aa9a3f31fbf3d161b.DomAK7AdgUDn3rtF" \
OPENAI_BASE_URL="https://api.z.ai/api/paas/v4" \
exec cargo run -p rusty-claude-cli -- --model glm-5.1 "$@"
