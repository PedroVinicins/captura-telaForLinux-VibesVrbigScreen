# Changelog

Todas as mudanças relevantes deste projeto são documentadas neste arquivo.

## [0.3.0] - 2026-09-10

### Adicionado

- Captura nativa de janelas no macOS 12.3 ou mais recente com ScreenCaptureKit.
- Seleção interativa de janelas no terminal ou por `VIBESVR_WINDOW`.
- Conversão validada de buffers BGRA do CoreVideo para frames RGBA.
- Dependências condicionais por sistema operacional, mantendo as pilhas de
  PipeWire e ScreenCaptureKit isoladas.
- Testes para stride, padding e buffers truncados no caminho de captura macOS.
- CI para formatação, análise estática, compilação e testes no Linux e macOS.

### Corrigido

- Inicialização do ScreenCaptureKit antes de o AppKit conectar-se ao
  WindowServer, que causava o abort `CGS_REQUIRE_INIT`.
- Seletor incorreto `setShowCursor:` da dependência `screen-capture-kit 0.7.1`;
  o aplicativo agora usa o setter nativo correto `setShowsCursor:`.
- Propagação de erros ocorridos durante a inicialização tardia da captura.

### Alterado

- A captura passa a iniciar no ciclo `Startup` do Bevy, depois da criação da
  janela nativa.
- Documentação consolidada para execução no Linux e no macOS.
- Remoção de backups, registros históricos e protótipos não compilados.

## [0.2.0] - 2026-09-09

### Alterado

- Pipeline PipeWire endurecido com validação de formatos, dimensões, buffers e
  metadados de frames.
- Cena 3D Side-by-Side e atualização de textura aprimoradas para baixa latência.

## [0.1.0] - 2026-09-09

- Primeira versão do capturador Linux com portal, PipeWire e ambiente SBS.

[0.3.0]: https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/PedroVinicins/captura-telaForLinux-VibesVrbigScreen/releases/tag/v0.2.0
