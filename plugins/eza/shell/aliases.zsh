# eza plugin — aliases.zsh
# All aliases use --color=always so colors survive piping to pagers.
# EZA_COLORS is set by lx init from the active theme's [ls_colors.columns].

alias ls='eza --color=auto --group-directories-first'
alias ll='eza -la --color=auto --group-directories-first --git --time-style=long-iso'
alias la='eza -a --color=auto --group-directories-first'
alias lt='eza -la --color=auto --group-directories-first --sort=modified'
alias tree='eza --tree --color=auto'
