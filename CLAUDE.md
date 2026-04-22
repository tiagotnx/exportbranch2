# CLAUDE.md

Contexto e diretrizes para o Claude trabalhar neste repositório.

## Sobre o projeto

`exportbranch` é uma CLI em Rust (edição 2021) que filtra, copia e converte
arquivos de uma branch de código Harbour antes da compilação pelo `Compex`.

- Crate binária única (`src/main.rs`).
- Dependência externa: `regex`.
- Suporte a Windows e Linux (há código sob `#[cfg(target_os = "...")]`).
- Módulos atuais: [configuration.rs](src/configuration.rs),
  [export.rs](src/export.rs), [export_branch.rs](src/export_branch.rs),
  [export_branch_files.rs](src/export_branch_files.rs),
  [file_checker.rs](src/file_checker.rs),
  [convert_file.rs](src/convert_file.rs),
  [convertions.rs](src/convertions.rs), [help.rs](src/help.rs).

## Comandos essenciais

```bash
cargo build              # debug
cargo build --release    # release
cargo test               # roda testes (unit + integração + doctests)
cargo test <padrão>      # filtra testes pelo nome
cargo test -- --nocapture        # mostra println! durante testes
cargo clippy -- -D warnings      # lint estrito (trata warning como erro)
cargo fmt                # formata o código
cargo fmt -- --check     # verifica formatação sem alterar
cargo check              # type-check rápido, sem gerar binário
```

Antes de declarar uma tarefa concluída, rode `cargo fmt`, `cargo clippy -- -D
warnings` e `cargo test`. Se algum falhar, corrija a causa raiz — não silencie
com `#[allow(...)]` nem com `--no-verify`.

## TDD é obrigatório

Toda mudança de comportamento segue o ciclo **Red → Green → Refactor**:

1. **Red** — escreva primeiro um teste que falhe expressando o comportamento
   desejado. Rode `cargo test` e confirme que ele falha pelo motivo esperado
   (não por erro de compilação acidental).
2. **Green** — escreva o **mínimo** de código de produção necessário para o
   teste passar. Não antecipe casos futuros.
3. **Refactor** — com a suíte verde, melhore nomes, elimine duplicação e
   simplifique. Rode `cargo test` após cada refator.

Regras práticas:

- Nunca adicione código de produção sem um teste falhando que o exija.
- Um teste por comportamento. Nomeie como `nome_do_cenario_resulta_em_X`
  (ex.: `build_sem_source_retorna_erro`).
- Bug fixes começam por um **teste de regressão** que reproduz o bug; só
  depois corrija.
- Refator puro (sem mudança de comportamento) não exige novo teste, mas exige
  que a suíte continue verde antes e depois.
- Se um teste é difícil de escrever, normalmente o design está errado —
  refatore para tornar o código testável (injeção de dependência, separar I/O
  de lógica) em vez de pular o teste.

### Onde colocar testes

- **Testes unitários** — no mesmo arquivo do código, em
  `#[cfg(test)] mod tests { ... }` no fim do arquivo. Use para lógica pura
  (ex.: `checked_to_regex`, `convert_buffer`, parsing de argumentos).
- **Testes de integração** — em `tests/` na raiz do crate, um arquivo por
  área (ex.: `tests/export.rs`). Use para fluxos end-to-end que tocam o
  sistema de arquivos.
- **I/O em testes** — use `tempfile::TempDir` (adicione `tempfile` em
  `[dev-dependencies]`) para criar diretórios isolados; nunca dependa de
  caminhos absolutos da máquina nem deixe lixo no repo.

Esqueleto:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_to_regex_converte_glob_em_regex() {
        let regex = checked_to_regex(vec!["*.prg".to_string()]);
        assert!(regex[0].is_match("foo.prg"));
        assert!(!regex[0].is_match("foo.txt"));
    }
}
```

## Boas práticas de Rust para este código

### Tratamento de erros

- **Não use `unwrap()` nem `expect()` em código de produção.** Hoje há vários
  pontos com `unwrap()` (ex.: [main.rs:55](src/main.rs#L55),
  [configuration.rs:95](src/configuration.rs#L95),
  [file_checker.rs:30](src/file_checker.rs#L30)). Ao tocar nesses arquivos,
  prefira propagar com `?` e retornar `Result`. `unwrap()` é aceitável apenas
  em testes.
- **Não use `std::process::exit` fora de `main`.** Funções de biblioteca devem
  retornar `Result<_, E>` e deixar o `main` decidir como reportar/encerrar.
- Prefira `Result<T, MeuErro>` com um enum de erro próprio (ou
  `Box<dyn std::error::Error>` em CLIs simples) a `String` como tipo de erro.
- Use `?` em vez de cadeias de `match` para propagação.

### Tipos e ownership

- Para **parâmetros**, prefira `&str` a `&String`, e `&Path` a `&PathBuf`.
  Aceite o tipo mais geral; retorne o tipo concreto.
- Evite `.clone()` defensivo — só clone quando o ownership realmente exigir.
  Vários `.clone()` no projeto podem desaparecer ao usar referências.
- Evite `Box<PathBuf>` (ex.: [export_branch.rs:8-9](src/export_branch.rs#L8-L9));
  `PathBuf` já é heap-alocado, o `Box` é redundante.
- Use `&[T]` em parâmetros em vez de `&Vec<T>`.

### Estilo e idiomas

- Siga `rustfmt` (sem opções customizadas — use o default).
- Resolva todos os lints de `cargo clippy -- -D warnings`.
- Nomeie em `snake_case` (funções/variáveis), `CamelCase` (tipos),
  `SCREAMING_SNAKE_CASE` (constantes).
- Use `if let` / `let else` em vez de `match` quando só um braço importa.
- Use iteradores (`.map`, `.filter`, `.collect`) em vez de loops manuais
  quando ficar mais claro.
- `format!("{var}")` (captura de variável no formato) é preferível a
  `format!("{}", var)` em strings novas.

### Estrutura

- Mantenha funções pequenas e com uma responsabilidade. Se um teste exige
  muito setup, divida a função.
- Separe **lógica pura** (testável sem I/O) de **efeitos** (leitura/escrita
  de arquivos, `println!`, `exit`). Lógica pura é onde o TDD rende mais.
- Constantes no topo do módulo, como já é feito em
  [configuration.rs](src/configuration.rs).
- Dependências em `Cargo.toml` com versão minor fixa (ex.: `regex = "1.9"`),
  não patch fixa, salvo motivo explícito.

### Multiplataforma

- Código específico de SO já usa `#[cfg(target_os = "...")]`. Mantenha esse
  padrão e, ao adicionar variantes, garanta que **todos** os alvos compilam
  (rode `cargo check` mentalmente para Windows e Linux).
- Use `std::path::MAIN_SEPARATOR` e `Path::join` em vez de concatenar `/`
  ou `\` manualmente.

## O que evitar

- Adicionar dependências sem necessidade clara — o projeto é enxuto.
- Reformatar arquivos não relacionados à mudança em curso.
- Criar abstrações para casos hipotéticos (traits, generics, builders) sem
  um teste que as exija agora.
- Comentários que descrevem o **que** o código faz; só comente o **porquê**
  quando não for óbvio.
- Mudanças amplas de estilo misturadas com mudanças de comportamento — faça
  em commits separados.
