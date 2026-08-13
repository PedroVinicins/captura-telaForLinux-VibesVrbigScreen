# VibesVR SBS

Este patch troca a visualização 2D do `pixels` por um cinema 3D no Bevy 0.14.2.

## O que ele faz

- Usa a captura Wayland/PipeWire que já existe no projeto.
- Atualiza a captura como textura de uma tela 3D 16:9.
- Renderiza dois olhos em Side-by-Side.
- Usa IPD de 0,064 m e FOV de 90 graus.
- Abre em tela cheia sem bordas para o Sunshine capturar.
- Não usa OpenH264: o Sunshine faz a codificação.

## Instalação

Extraia este patch na raiz de `~/captura-tela`, preservando os arquivos atuais de captura:

```bash
cd ~/captura-tela
cp Cargo.toml Cargo.toml.backup
cp src/main.rs src/main.rs.backup
unzip -o ~/Downloads/vibesvr-sbs-patch.zip -d ~/captura-tela
cargo check
cargo run --release
```

Quando o portal abrir, selecione o monitor do Fedora que será mostrado na tela 3D.

## Controles

- `Esc`: sair.
- `F11`: alternar tela cheia/janela.

## Sunshine e Moonlight

1. No Sunshine, use resolução de `1920x1080` e `60 FPS`.
2. Adicione um aplicativo chamado `VibesVR` apontando para:

```bash
/home/pedrosilva/captura-tela/target/release/captura-tela-video
```

3. Abra `VibesVR` pelo Moonlight.
4. Deixe o celular em modo paisagem e coloque-o no visor VR.

## Evitar efeito de espelho infinito

O monitor capturado pelo portal não pode ser o mesmo monitor que mostra a janela SBS. Use uma segunda saída/monitor para o VibesVR. Para testar com apenas uma tela, selecione uma janela específica como fonte em vez do monitor inteiro; isso exige habilitar `SourceType::Window` no `portal.rs`.
