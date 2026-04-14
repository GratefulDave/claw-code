#!/usr/bin/env bash
export OPENAI_API_KEY="ae5c1598a2d4454aa9a3f31fbf3d161b.DomAK7AdgUDn3rtF"
export OPENAI_BASE_URL="https://api.z.ai/api/coding/paas/v4"
exec /Users/davidandrews/PycharmProjects/claw-code/rust/target/debug/claw --model "openai/glm-5.1" "$@"
