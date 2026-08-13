# VibesVR Screen Capture

Aplicativo Linux em Rust que captura uma janela autorizada pelo
`xdg-desktop-portal`, recebe seus frames pelo PipeWire e os exibe em um cinema
3D Side-by-Side com Bevy. Sunshine/Moonlight é responsável por codificar e
transmitir a janela final; este executável não inicia um servidor de vídeo.

## Arquitetura

```text
main/Tokio
  └─ xdg-desktop-portal ── autorização, sessão e descritor PipeWire
       └─ PipeWire ── negociação e buffers RAW
            └─ conversão para Frame RGBA
                 └─ canal limitado (2 frames)
                      └─ Bevy/wgpu ── textura do cinema e câmeras SBS
```

- `src/portal.rs`: integração segura com o portal de screencast.
- `src/pipewire/stream.rs`: stream, negociação e conversão dos pixels.
- `src/capture.rs`: ciclo de vida conjunto da sessão e do stream.
- `src/frame.rs`: frame imutável com metadados.
- `src/vr.rs`: cena 3D, textura, câmeras e controles.

Os arquivos em `src/encoder/` e os módulos PipeWire não declarados por
`src/pipewire/mod.rs` são protótipos e não fazem parte do binário atual.

## Dependências de sistema

É necessário ter Rust e os pacotes de desenvolvimento do PipeWire/SPA
instalados. Os nomes variam conforme a distribuição; no Fedora normalmente são
`pipewire-devel` e `clang-devel`.

## Executar

```bash
cargo run --release
```

Use `--release` para captura real. O perfil debug torna a conversão de mais de
dois milhões de pixels por frame muito lenta e pode aparentar travamentos.

Quando o seletor do portal aparecer, escolha a janela que será exibida. A
aplicação começa em modo janela para reduzir o risco de capturar a própria saída.

Controles:

- `Esc`: sair.
- `F11`: alternar entre janela e tela cheia sem bordas.

## Desenvolvimento

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

O processamento descarta frames antigos quando a renderização está atrasada,
priorizando baixa latência. São aceitos frames RGBA de até 128 MiB e resolução
máxima negociada de 8192×8192.

## Limitações atuais

- Apenas captura de janela está habilitada.
- Buffers DMA-BUF que não podem ser mapeados pela CPU são descartados.
- Não há rastreamento de cabeça; as duas câmeras têm posição fixa.
- O encoder H.264 presente como protótipo não está conectado ao aplicativo.
- A integração gráfica precisa ser testada dentro de uma sessão Wayland/X11
  com portal e PipeWire ativos.
