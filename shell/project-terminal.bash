# Project Terminal - Bash OSC 133 Shell Entegrasyonu
# Kullanıcının standart bashrc ayarlarını yükle
if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi

__term_osc133_prompt_start() {
    printf "\033]133;A\007"
}

__term_osc133_command_start() {
    printf "\033]133;B\007"
}

__term_osc133_preexec() {
    printf "\033]133;C\007"
}

__term_osc133_postexec() {
    local exit_code=$?
    printf "\033]133;D;%d\007" "$exit_code"
    # OSC 7: CWD güncelleme
    printf "\033]7;file://%s%s\007" "$HOSTNAME" "$PWD"
}

# Bash PROMPT_COMMAND bağlaması
if [ -z "$PROMPT_COMMAND" ]; then
    PROMPT_COMMAND="__term_osc133_postexec; __term_osc133_prompt_start"
else
    PROMPT_COMMAND="__term_osc133_postexec; __term_osc133_prompt_start; $PROMPT_COMMAND"
fi

PS1="\[\033]133;B\007\]$PS1"

trap '__term_osc133_preexec' DEBUG
