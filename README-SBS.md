# VibesVR SBS — guia rápido

O modo cinema 3D Side-by-Side funciona no Linux e no macOS e já faz parte do
binário principal. Consulte o [README.md](README.md) para requisitos, permissões
de captura e instruções completas.

```bash
cargo run --release
```

No Linux, escolha a janela no portal. No macOS, escolha uma janela na lista do
terminal ou use, por exemplo:

```bash
VIBESVR_WINDOW="Firefox" cargo run --release
```

No Sunshine, configure uma aplicação apontando para o executável produzido em
`target/release/captura-tela-video`, normalmente com resolução 1920×1080 e 60
FPS. Abra-a pelo Moonlight e use o aparelho em modo paisagem no visor VR.

Para evitar o efeito de espelho infinito, não selecione a própria janela do
VibesVR como fonte.

Controles principais: `Esc` sai e `F11` alterna tela cheia. Os demais controles
de lente e distância estão documentados no README principal.
