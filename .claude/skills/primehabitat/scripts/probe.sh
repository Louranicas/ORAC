#!/usr/bin/env bash
# Thin wrapper around habitat-probe for skill scripts/ convention.
# Run with --help to see all commands.
exec habitat-probe "${@:-pulse}"
