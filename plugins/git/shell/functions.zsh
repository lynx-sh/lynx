git_branch() {
  git symbolic-ref --short HEAD 2>/dev/null
}

git_dirty() {
  [[ -n "$(git status --porcelain 2>/dev/null)" ]] && echo "1" || echo "0"
}

git_stash_count() {
  git stash list 2>/dev/null | wc -l | tr -d ' '
}
