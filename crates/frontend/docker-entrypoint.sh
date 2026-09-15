#!/bin/sh
# Renders nginx.conf.template -> conf.d/default.conf with just $PORT
# substituted (Railway injects PORT; 8080 is the local-docker-run
# fallback). Deliberately not the nginx image's built-in template
# auto-substitution — that runs envsubst with no variable allowlist, so
# it would also mangle nginx's own $uri in the template.
set -eu
export PORT="${PORT:-8080}"
envsubst '$PORT' < /etc/nginx/templates/default.conf.template > /etc/nginx/conf.d/default.conf
exec nginx -g 'daemon off;'
