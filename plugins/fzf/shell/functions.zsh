# fzf plugin — functions.zsh

# Interactive git log browser — preview shows commit details
fzf_git_log() {
  git log --oneline --color=always 2>/dev/null \
    | fzf --ansi --no-sort --reverse --tiebreak=index \
        --preview 'git show --stat --color=always {1}' \
        --preview-window=right:60% \
        --bind 'enter:execute(git show --color=always {1} | less -R)'
}

# Interactive branch switcher — checks out selected branch
fzf_git_branch() {
  local branch
  branch=$(git branch --all --color=never 2>/dev/null \
    | grep -v 'HEAD' \
    | sed 's|remotes/origin/||' \
    | sort -u \
    | fzf --prompt="branch> " --height=40%)
  [[ -n "$branch" ]] && git switch "${branch// /}"
}
