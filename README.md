# VibesVR Screen Capture

[![CI](https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/actions/workflows/ci.yml/badge.svg)](https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/PedroVinicins/captura-telaForLinux-VibesVrbigScreen)](https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/releases/latest)

Aplicativo em Rust para Linux e macOS que captura uma janela e a exibe em um
cinema 3D Side-by-Side com Bevy. Sunshine/Moonlight pode codificar e transmitir
a janela final; este executável não inicia um servidor de vídeo.

## Arquitetura

```text
                        ┌─ Linux: xdg-desktop-portal → PipeWire
main → captura nativa ──┤
                        └─ macOS: ScreenCaptureKit → CoreVideo
                                      │
                                      ▼
                              Frame RGBA validado
                                      │
                              canal limitado (2)
                                      │
                                      ▼
                     Bevy/wgpu → cinema 3D + câmeras SBS
```

- `src/capture/linux.rs`: ciclo de vida do portal e do PipeWire no Linux.
- `src/capture/macos.rs`: seleção e captura contínua de janela com
  ScreenCaptureKit no macOS.
- `src/capture.rs`: fachada comum escolhida em tempo de compilação.
- `src/frame.rs`: frame RGBA imutável com metadados.
- `src/vr.rs`: cena 3D, textura, câmeras e controles compartilhados.

## Requisitos

### macOS

- macOS 12.3 ou mais recente.
- Rust estável.
- Xcode Command Line Tools (`xcode-select --install`).
- Permissão de **Gravação de Tela** em **Ajustes do Sistema → Privacidade e
  Segurança**. Em versões recentes, a opção pode aparecer como **Gravação de
  Tela e Áudio do Sistema**.

Na primeira execução, aceite a solicitação do macOS, encerre o programa e abra-o
novamente. Ao usar `cargo run`, autorize também o Terminal/iTerm utilizado para
iniciar o processo, se ele aparecer na lista de permissões.

### Linux

É necessário ter um ambiente Wayland ou X11 com `xdg-desktop-portal`, além dos
pacotes de desenvolvimento do PipeWire/SPA. No Fedora, normalmente:

```bash
sudo dnf install pipewire-devel clang-devel
```

No Ubuntu/Debian, normalmente:

```bash
sudo apt install libpipewire-0.3-dev libspa-0.2-dev clang
```

## Compilar e executar

O mesmo comando funciona nos dois sistemas:

```bash
cargo run --release
```

Use `--release` para captura real. O perfil debug torna a conversão de milhões
de pixels por frame muito lenta.

No Linux, escolha a janela no seletor gráfico do portal. No macOS, escolha o
número da janela na lista exibida no terminal. Para selecionar sem interação,
defina `VIBESVR_WINDOW` com um trecho do título, nome do aplicativo ou ID:

```bash
VIBESVR_WINDOW="Firefox" cargo run --release
```

A aplicação começa em modo janela para reduzir o risco de capturar a própria
saída.

## Controles

- `Esc`: sair.
- `F11`: alternar entre janela e tela cheia sem bordas.
- `Q` / `E`: diminuir/aumentar o zoom da tela (0,50× a 2,00×).
- `Z` / `X`: diminuir/aumentar o FOV das lentes (55° a 120°).
- `C` / `V`: diminuir/aumentar o IPD (50 mm a 78 mm).
- `R` / `F`: aproximar/afastar a tela virtual (3 m a 12 m).
- `0`: restaurar zoom, FOV, IPD e distância padrão.

Os ajustes são aplicados simultaneamente aos dois olhos. Ajuste o IPD com
cuidado para evitar desconforto visual; comece próximo de 64 mm.

## Desenvolvimento

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

O histórico de mudanças está em [CHANGELOG.md](CHANGELOG.md). As versões
publicadas podem ser encontradas na página de
[releases](https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/releases).

As dependências específicas de Linux e macOS ficam em seções condicionais do
`Cargo.toml`: uma plataforma não precisa instalar nem compilar a pilha nativa da
outra. O processamento aceita frames RGBA de até 128 MiB e 8192×8192, mantém no
máximo dois frames prontos e prioriza sempre o frame mais recente.

## Limitações atuais

- Apenas captura de janela está habilitada.
- No Linux, buffers DMA-BUF que não podem ser mapeados pela CPU são descartados.
- Não há rastreamento de cabeça; as duas câmeras têm posição fixa.
- O teste visual completo exige uma sessão gráfica e a permissão de captura do
  sistema operacional.
