#!/bin/bash
# Usage: Tools/downloads.sh
#
# Prints the GitHub download count of each release file of Insert and of the Windows port.
# Sparkle downloads Insert-Update.dmg, so that file counts updates. Each other file counts new installs.
set -euo pipefail

# The rows are read in full first, so a failed request stops the script before it prints totals.
rows=""
for repo in KenWuqianghao/Insert KenWuqianghao/insert-windows; do
	rows+=$(gh api --paginate "repos/$repo/releases" \
		--jq ".[] | .tag_name as \$tag | .assets[] | [\"$repo\", \$tag, .name, .download_count] | @tsv")$'\n'
done

awk -F '\t' '
	BEGIN { printf "%-8s  %-8s  %-34s  %9s\n", "Platform", "Release", "File", "Downloads" }
	NF {
		platform = ($1 ~ /insert-windows$/) ? "Windows" : "macOS"
		kind = ($3 == "Insert-Update.dmg") ? "updates" : "installs"
		printf "%-8s  %-8s  %-34s  %9d\n", platform, $2, $3, $4
		total[platform " " kind] += $4
	}
	END {
		print ""
		printf "macOS installs:    %d\n", total["macOS installs"]
		printf "macOS updates:     %d\n", total["macOS updates"]
		printf "Windows installs:  %d\n", total["Windows installs"]
		printf "All installs:      %d\n", total["macOS installs"] + total["Windows installs"]
	}' <<<"$rows"
