#!/usr/bin/env bash
# Generate shell completions for the copypaste CLI.
# Usage: source <(bash scripts/completions.sh bash)
#        bash scripts/completions.sh zsh > ~/.zsh/_copypaste

SHELL_TYPE="${1:-bash}"

case "$SHELL_TYPE" in
bash)
cat << 'EOF'
_copypaste_completions() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local commands="list count status delete search copy watch export clear stats import pin --help --version"
    COMPREPLY=($(compgen -W "$commands" -- "$cur"))
}
complete -F _copypaste_completions copypaste
EOF
;;
zsh)
cat << 'EOF'
#compdef copypaste
_copypaste() {
    local -a cmds
    cmds=(
        'list:List clipboard history'
        'count:Show total item count'
        'status:Check daemon status'
        'delete:Delete an item by ID'
        'search:Full-text search history'
        'copy:Copy item back to clipboard'
        'watch:Watch new items in real-time'
        'export:Export history as JSON'
        'clear:Clear all history'
        'stats:Show statistics'
        'import:Import from JSON file'
        'pin:Pin item to prevent expiry'
    )
    _describe 'command' cmds
}
_copypaste
EOF
;;
fish)
cat << 'EOF'
complete -c copypaste -f
complete -c copypaste -n '__fish_use_subcommand' -a list -d 'List history'
complete -c copypaste -n '__fish_use_subcommand' -a count -d 'Total count'
complete -c copypaste -n '__fish_use_subcommand' -a status -d 'Daemon status'
complete -c copypaste -n '__fish_use_subcommand' -a delete -d 'Delete item'
complete -c copypaste -n '__fish_use_subcommand' -a search -d 'Search history'
complete -c copypaste -n '__fish_use_subcommand' -a copy -d 'Copy to clipboard'
complete -c copypaste -n '__fish_use_subcommand' -a watch -d 'Watch live'
complete -c copypaste -n '__fish_use_subcommand' -a export -d 'Export JSON'
complete -c copypaste -n '__fish_use_subcommand' -a clear -d 'Clear history'
complete -c copypaste -n '__fish_use_subcommand' -a stats -d 'Statistics'
complete -c copypaste -n '__fish_use_subcommand' -a import -d 'Import JSON'
complete -c copypaste -n '__fish_use_subcommand' -a pin -d 'Pin item'
EOF
;;
*)
    echo "Usage: $0 [bash|zsh|fish]" >&2
    exit 1
    ;;
esac
