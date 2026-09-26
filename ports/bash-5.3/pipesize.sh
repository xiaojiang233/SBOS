#!/bin/sh
# Generate builtins/pipesize.h for the SBOS target.
#
# Upstream derives this value by building and running psize.aux, a host program
# that writes into a pipe until it blocks, and then reports how much it wrote.
# That measures the build machine's pipe buffer, which says nothing about SBOS,
# and the program cannot be run at all during a cross build. SBOS pipes have a
# fixed capacity, so the value comes from the kernel instead: see CAPACITY in
# kernel/src/ipc/pipe.rs.

cat <<'PIPESIZE_HEADER'
/*
 * pipesize.h
 *
 * Generated for SBOS by ports/bash-5.3/pipesize.sh.
 * PIPESIZE is the pipe capacity implemented by the SBOS kernel and is used by
 * the ulimit builtin to report `ulimit -p`.
 */
#define PIPESIZE 4096
PIPESIZE_HEADER
