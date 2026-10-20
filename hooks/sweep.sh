#!/bin/sh
# shello: deletes recordings older than 7 days whose shell has exited.
# Normally a session's files are removed when its terminal closes; this
# catches the ones left behind by crashes or killed terminals.
#
# Usage: sweep.sh SESSIONS_DIR

dir=$1

find "$dir" -type f -mtime +7 2>/dev/null | while IFS= read -r file; do
	name=$(basename "$file")
	id=${name%.*}
	pid=${id##*-}
	# Session ids end in the recording shell's PID; skip ones still running.
	kill -0 "$pid" 2>/dev/null && continue
	rm -f "$dir/$id".log "$dir/$id".cmds "$dir/$id".started
done
