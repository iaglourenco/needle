# Hook do Needle

O Needle se auto-configura: na primeira vez que o app abre, ele registra a
si mesmo (`<caminho-do-executável> hook`) em `~/.claude/settings.json` para
todos os eventos relevantes (`SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `PostToolUse`, `Notification`, `Stop`, `SubagentStop`,
`SessionEnd`, `PreCompact`). Isso é idempotente — roda toda vez que o app
sobe e corrige o caminho sozinho se o app for movido ou atualizado.

Não é preciso Node.js nem nenhum script externo: o próprio binário do
Needle, chamado com o argumento `hook`, lê o payload do stdin e repassa pro
app em execução via HTTP local.

## Desinstalação

O desinstalador (NSIS) roda `needle.exe remove-hooks` automaticamente antes
de apagar o executável — as entradas do Needle saem de
`~/.claude/settings.json` sozinhas, sem deixar hook morto configurado.

## Reconfigurar ou remover

Pelo ícone da bandeja: **Reconfigurar hooks** / pela tela de Configurações:
botão **Remover**. Ambos alteram só as entradas do Needle — hooks de outras
ferramentas em `settings.json` não são tocados (é feito um backup em
`settings.json.needle-backup` antes de qualquer escrita).

## Configuração manual (projeto específico, ou auto-config desabilitada)

Se preferir registrar o hook só num projeto (`.claude/settings.json` do
repositório) em vez de globalmente, adicione manualmente, trocando o
caminho pelo local real do executável instalado:

```json
{
  "hooks": {
    "Notification": [
      { "hooks": [{ "type": "command", "command": "\"C:\\Program Files\\Needle\\needle.exe\" hook" }] }
    ]
  }
}
```

Repita para os demais eventos listados acima conforme necessário.
