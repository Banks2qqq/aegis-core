#!/usr/bin/env bash
# Shared SSH connection — password asked once. Source: . deploy/ssh-mux.sh
set -euo pipefail

: "${VPS_HOST:=178.236.16.101}"
: "${VPS_USER:=root}"

SSH_CTL="${SSH_CONTROL_PATH:-/tmp/aegis-ssh-${VPS_USER}-${VPS_HOST}.sock}"
export SSH_CONTROL_PATH="$SSH_CTL"

_ssh_base() {
  local -a opts=(-o StrictHostKeyChecking=no -o ControlPath="$SSH_CTL")
  if [[ -n "${VPS_PASSWORD:-}" ]] && command -v sshpass >/dev/null 2>&1; then
    SSHPASS="$VPS_PASSWORD" sshpass -e ssh "${opts[@]}" "$@"
  else
    ssh "${opts[@]}" "$@"
  fi
}

_scp_base() {
  local -a opts=(-o StrictHostKeyChecking=no -o ControlPath="$SSH_CTL")
  if [[ -n "${VPS_PASSWORD:-}" ]] && command -v sshpass >/dev/null 2>&1; then
    SSHPASS="$VPS_PASSWORD" sshpass -e scp "${opts[@]}" "$@"
  else
    scp "${opts[@]}" "$@"
  fi
}

ssh_mux_open() {
  if ssh -o ControlPath="$SSH_CTL" -O check "${VPS_USER}@${VPS_HOST}" >/dev/null 2>&1; then
    return 0
  fi
  echo "==> SSH to ${VPS_USER}@${VPS_HOST} (password only once; Ctrl+C to abort)"
  if [[ -n "${VPS_PASSWORD:-}" ]] && ! command -v sshpass >/dev/null 2>&1; then
    echo "    Tip: brew install hudochenkov/sshpass/sshpass  — then export VPS_PASSWORD works without typing"
  fi
  _ssh_base -o ControlMaster=yes -o ControlPersist=600 \
    "${VPS_USER}@${VPS_HOST}" "echo connected"
}

ssh_mux_close() {
  ssh -o ControlPath="$SSH_CTL" -O exit "${VPS_USER}@${VPS_HOST}" 2>/dev/null || true
}

ssh_cmd() {
  _ssh_base "${VPS_USER}@${VPS_HOST}" "$@"
}

scp_cmd() {
  _scp_base "$@"
}

scp_dir_cmd() {
  _scp_base -r "$@"
}
